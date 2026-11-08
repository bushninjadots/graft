use serde::Serialize;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolInfo {
    pub id: String,
    pub name: String,
    pub found: bool,
    pub version: Option<String>,
    pub path: Option<String>,
}

struct ToolSpec {
    id: &'static str,
    name: &'static str,
    version_args: &'static [&'static str],
}

const TOOLS: &[ToolSpec] = &[
    ToolSpec { id: "git", name: "Git", version_args: &["--version"] },
    ToolSpec { id: "gh", name: "GitHub CLI", version_args: &["--version"] },
    ToolSpec { id: "rustc", name: "Rust", version_args: &["--version"] },
    ToolSpec { id: "cargo", name: "Cargo", version_args: &["--version"] },
    ToolSpec { id: "node", name: "Node.js", version_args: &["--version"] },
    ToolSpec { id: "npm", name: "npm", version_args: &["--version"] },
    ToolSpec { id: "pnpm", name: "pnpm", version_args: &["--version"] },
    ToolSpec { id: "bun", name: "Bun", version_args: &["--version"] },
    ToolSpec { id: "python3", name: "Python", version_args: &["--version"] },
    ToolSpec { id: "go", name: "Go", version_args: &["version"] },
    ToolSpec { id: "sqlite3", name: "SQLite", version_args: &["--version"] },
    ToolSpec { id: "docker", name: "Docker", version_args: &["--version"] },
    ToolSpec { id: "podman", name: "Podman", version_args: &["--version"] },
    ToolSpec { id: "ssh", name: "OpenSSH", version_args: &["-V"] },
    ToolSpec { id: "gpg", name: "GnuPG", version_args: &["--version"] },
    ToolSpec { id: "fish", name: "fish", version_args: &["--version"] },
    ToolSpec { id: "zsh", name: "zsh", version_args: &["--version"] },
    ToolSpec { id: "hyprctl", name: "Hyprland", version_args: &["version"] },
];

pub fn detect() -> Vec<ToolInfo> {
    TOOLS
        .iter()
        .map(|spec| {
            let path = which(spec.id);
            let version = path.as_deref().and_then(|p| version_of(p, spec.version_args));
            ToolInfo {
                id: spec.id.to_string(),
                name: spec.name.to_string(),
                found: path.is_some(),
                version,
                path,
            }
        })
        .collect()
}

fn which(binary: &str) -> Option<String> {
    let paths = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(binary);
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
    }
    None
}

fn version_of(binary: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(binary).args(args).output().ok()?;
    let raw = if output.stdout.is_empty() {
        String::from_utf8_lossy(&output.stderr).to_string()
    } else {
        String::from_utf8_lossy(&output.stdout).to_string()
    };
    let line = raw.lines().map(str::trim).find(|l| !l.is_empty())?;
    Some(collapse(line))
}

fn collapse(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut last_space = false;
    for ch in line.chars() {
        if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(ch);
            last_space = false;
        }
    }
    let trimmed = out.trim();
    if trimmed.chars().count() > 72 {
        let cut: String = trimmed.chars().take(72).collect();
        format!("{cut}…")
    } else {
        trimmed.to_string()
    }
}
