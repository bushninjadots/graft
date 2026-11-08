//! Real terminal sessions for GRAFT. Every PTY is owned by Rust; the webview
//! only sends keystrokes and receives output through a Tauri channel.

use portable_pty::{native_pty_system, Child, CommandBuilder, MasterPty, PtySize};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use tauri::ipc::Channel;
use tauri::State;

static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct TerminalManager {
    sessions: Mutex<HashMap<String, Session>>,
}

struct Session {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    channel: Channel<TerminalEvent>,
}

#[derive(Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TerminalEvent {
    Output { data: String },
    Exit,
    Error { message: String },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalSessionInfo {
    pub id: String,
    pub shell: String,
    pub cwd: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalEnvironment {
    pub cwd: String,
    pub shells: Vec<String>,
    pub default_shell: String,
}

fn home_dir() -> String {
    std::env::var("HOME").unwrap_or_else(|_| ".".to_string())
}

fn find_in_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for base in std::env::split_paths(&path) {
        let candidate = base.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

fn available_shells() -> Vec<String> {
    let mut seen = HashSet::new();
    let mut shells = Vec::new();
    for name in ["fish", "bash", "zsh", "sh"] {
        if let Some(path) = find_in_path(name) {
            let text = path.display().to_string();
            if seen.insert(text.clone()) {
                shells.push(text);
            }
        }
    }
    if let Ok(shell) = std::env::var("SHELL") {
        if Path::new(&shell).is_file() && seen.insert(shell.clone()) {
            shells.insert(0, shell);
        }
    }
    shells
}

fn default_shell(shells: &[String]) -> String {
    shells
        .iter()
        .find(|shell| shell.ends_with("/fish"))
        .cloned()
        .or_else(|| shells.first().cloned())
        .or_else(|| std::env::var("SHELL").ok())
        .unwrap_or_else(|| "/bin/sh".to_string())
}

fn valid_cwd(raw: &str) -> Result<String, String> {
    let value = if raw.trim().is_empty() {
        home_dir()
    } else {
        raw.trim().to_string()
    };
    let path = Path::new(&value);
    if !path.is_dir() {
        return Err(format!("Terminal directory does not exist: {value}"));
    }
    Ok(path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string())
}

fn valid_shell(raw: &str, shells: &[String]) -> Result<String, String> {
    if raw.trim().is_empty() {
        return Ok(default_shell(shells));
    }
    let path = Path::new(raw.trim());
    if !path.is_file() {
        return Err("That shell is not an executable file.".to_string());
    }
    if !shells.iter().any(|shell| Path::new(shell) == path) {
        return Err("Choose one of the shells GRAFT detected on PATH.".to_string());
    }
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn terminal_environment() -> TerminalEnvironment {
    let shells = available_shells();
    TerminalEnvironment {
        cwd: home_dir(),
        default_shell: default_shell(&shells),
        shells,
    }
}

/// Starts a PTY. The caller must have created a channel before invoking this;
/// output can arrive immediately, so the frontend buffers until xterm mounts.
#[tauri::command]
pub fn terminal_start(
    manager: State<'_, TerminalManager>,
    channel: Channel<TerminalEvent>,
    cwd: String,
    shell: String,
    cols: u16,
    rows: u16,
) -> Result<TerminalSessionInfo, String> {
    let shells = available_shells();
    let cwd = valid_cwd(&cwd)?;
    let shell = valid_shell(&shell, &shells)?;
    let cols = cols.clamp(2, 500);
    let rows = rows.clamp(2, 500);

    let pair = native_pty_system()
        .openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Cannot open a PTY: {error}"))?;

    let mut command = CommandBuilder::new(&shell);
    command.cwd(&cwd);
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    command.env("GRAFT_TERMINAL", "1");
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| format!("Cannot start {shell}: {error}"))?;
    drop(pair.slave);

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| format!("Cannot read the terminal: {error}"))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|error| format!("Cannot write to the terminal: {error}"))?;

    let id = format!(
        "term-{}-{}",
        std::process::id(),
        NEXT_SESSION.fetch_add(1, Ordering::Relaxed)
    );
    let thread_channel = channel.clone();
    std::thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let data = String::from_utf8_lossy(&buffer[..count]).to_string();
                    if thread_channel
                        .send(TerminalEvent::Output { data })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(error) => {
                    let _ = thread_channel.send(TerminalEvent::Error {
                        message: format!("terminal read failed: {error}"),
                    });
                    break;
                }
            }
        }
        let _ = thread_channel.send(TerminalEvent::Exit);
    });

    let info = TerminalSessionInfo {
        id: id.clone(),
        shell,
        cwd,
    };
    manager
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .insert(
            id,
            Session {
                master: pair.master,
                writer,
                child,
                channel,
            },
        );
    Ok(info)
}

#[tauri::command]
pub fn terminal_write(
    manager: State<'_, TerminalManager>,
    session_id: String,
    data: String,
) -> Result<(), String> {
    if data.len() > 128_000 {
        return Err("Terminal input is too large.".to_string());
    }
    let mut sessions = manager
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Terminal session is no longer running.".to_string())?;
    session
        .writer
        .write_all(data.as_bytes())
        .and_then(|_| session.writer.flush())
        .map_err(|error| format!("Cannot write to the terminal: {error}"))
}

#[tauri::command]
pub fn terminal_resize(
    manager: State<'_, TerminalManager>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    let mut sessions = manager
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Terminal session is no longer running.".to_string())?;
    session
        .master
        .resize(PtySize {
            rows: rows.clamp(2, 500),
            cols: cols.clamp(2, 500),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|error| format!("Cannot resize the terminal: {error}"))
}

#[tauri::command]
pub fn terminal_close(
    manager: State<'_, TerminalManager>,
    session_id: String,
) -> Result<(), String> {
    let mut sessions = manager
        .sessions
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let Some(mut session) = sessions.remove(&session_id) else {
        return Ok(());
    };
    let _ = session.child.kill();
    let _ = session.channel.send(TerminalEvent::Exit);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_directories_and_shells() {
        assert!(valid_cwd("/this/path/does/not/exist/graft").is_err());
        assert!(valid_cwd("").is_ok());
        assert!(valid_shell("/this/shell/does/not/exist", &[]).is_err());
        assert!(valid_shell("", &[]).is_ok());
    }

    #[test]
    fn detects_at_least_one_shell_on_this_machine() {
        let shells = available_shells();
        assert!(!shells.is_empty(), "no shell found on PATH");
        assert!(shells.iter().all(|shell| Path::new(shell).is_file()));
    }
}
