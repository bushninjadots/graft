//! Read-only inspection of a linked project directory through the Git CLI.
//! Nothing here writes to a repository or changes the user's working tree.

use serde::Serialize;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub path: String,
    pub is_repo: bool,
    pub toplevel: Option<String>,
    pub branch: Option<String>,
    pub detached: bool,
    pub ahead: u32,
    pub behind: u32,
    pub changed: u32,
    pub remote: Option<String>,
    /// `owner/repo` when the remote is a GitHub URL, otherwise `None`.
    pub github: Option<String>,
    pub error: Option<String>,
}

impl GitStatus {
    fn blank(path: &str, error: Option<String>) -> Self {
        Self {
            path: path.to_string(),
            is_repo: false,
            toplevel: None,
            branch: None,
            detached: false,
            ahead: 0,
            behind: 0,
            changed: 0,
            remote: None,
            github: None,
            error,
        }
    }
}

fn git(path: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Git is not installed or not on PATH.".to_string()
            } else {
                format!("Cannot run git: {e}")
            }
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().next().unwrap_or("git failed").trim();
        return Err(detail.to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Inspects `path` and never fails: a directory that is not a repository simply
/// reports `is_repo: false`.
pub fn status(path: &str) -> GitStatus {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return GitStatus::blank(path, None);
    }
    if !Path::new(trimmed).is_dir() {
        return GitStatus::blank(
            path,
            Some(format!("Directory does not exist: {trimmed}")),
        );
    }

    let toplevel = match git(trimmed, &["rev-parse", "--show-toplevel"]) {
        Ok(value) => value.trim().to_string(),
        Err(error) => return GitStatus::blank(path, Some(error)),
    };

    let porcelain = git(
        trimmed,
        &["status", "--porcelain=v2", "--branch", "--untracked-files=normal"],
    )
    .unwrap_or_default();
    let (branch, detached, ahead, behind, changed) = parse_status(&porcelain);

    let remote = git(trimmed, &["config", "--get", "remote.origin.url"])
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            git(trimmed, &["remote"])
                .ok()
                .and_then(|list| list.lines().next().map(str::trim).map(str::to_string))
                .filter(|value| !value.is_empty())
        });
    let github = remote.as_deref().and_then(github_slug);

    GitStatus {
        path: path.to_string(),
        is_repo: true,
        toplevel: Some(toplevel),
        branch,
        detached,
        ahead,
        behind,
        changed,
        remote,
        github,
        error: None,
    }
}

/// Parses `git status --porcelain=v2 --branch` output.
fn parse_status(text: &str) -> (Option<String>, bool, u32, u32, u32) {
    let mut branch: Option<String> = None;
    let mut oid: Option<String> = None;
    let mut detached = false;
    let mut ahead = 0;
    let mut behind = 0;
    let mut changed = 0;

    for line in text.lines() {
        if let Some(head) = line.strip_prefix("# branch.head ") {
            let head = head.trim();
            if head == "(detached)" {
                detached = true;
            } else if !head.is_empty() {
                branch = Some(head.to_string());
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.oid ") {
            let value = value.trim();
            if value != "(initial)" {
                oid = Some(value.to_string());
            }
            continue;
        }
        if let Some(value) = line.strip_prefix("# branch.ab ") {
            for token in value.split_whitespace() {
                if let Some(count) = token.strip_prefix('+') {
                    ahead = count.parse().unwrap_or(0);
                } else if let Some(count) = token.strip_prefix('-') {
                    behind = count.parse().unwrap_or(0);
                }
            }
            continue;
        }
        if line.starts_with("# ") || line.is_empty() {
            continue;
        }
        changed += 1;
    }

    if detached {
        branch = oid.map(|sha| sha.chars().take(7).collect());
    }

    (branch, detached, ahead, behind, changed)
}

/// Extracts `owner/repo` from the common GitHub remote URL shapes.
pub fn github_slug(remote: &str) -> Option<String> {
    let trimmed = remote.trim().trim_end_matches('/');
    let without_suffix = trimmed.strip_suffix(".git").unwrap_or(trimmed);

    let host_at = without_suffix.find("github.com")?;
    let mut rest = &without_suffix[host_at + "github.com".len()..];
    rest = rest.trim_start_matches([':', '/']);

    // Bare SSH remotes may carry a port: ssh://git@github.com:22/owner/repo
    if let Some(stripped) = rest.strip_prefix(|c: char| c.is_ascii_digit()) {
        if let Some(slash) = stripped.find('/') {
            let (port, remainder) = stripped.split_at(slash);
            if port.chars().all(|c| c.is_ascii_digit()) {
                rest = remainder.trim_start_matches('/');
            }
        }
    }

    let mut parts = rest.split('/');
    let owner = parts.next()?.trim();
    let repo = parts.next()?.trim();

    let valid = |value: &str| {
        !value.is_empty()
            && value
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };

    if valid(owner) && valid(repo) {
        Some(format!("{owner}/{repo}"))
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitActionResult {
    pub action: String,
    pub ok: bool,
    pub output: String,
    pub error: Option<String>,
}

/// Runs one of the explicitly allowed write operations against a linked project.
/// Anything else is rejected; there is no generic command runner here.
pub fn action(path: &str, action: &str) -> GitActionResult {
    let args: &[&str] = match action {
        "fetch" => &["fetch", "--all", "--prune"],
        "pull" => &["pull", "--ff-only"],
        "push" => &["push"],
        other => {
            return GitActionResult {
                action: other.to_string(),
                ok: false,
                output: String::new(),
                error: Some(format!("Unsupported git action: {other}")),
            }
        }
    };

    if !Path::new(path).is_dir() {
        return GitActionResult {
            action: action.to_string(),
            ok: false,
            output: String::new(),
            error: Some(format!("Directory does not exist: {path}")),
        };
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .output();

    match output {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&stderr);
            }
            let text = truncate_output(text.trim());
            if output.status.success() {
                GitActionResult {
                    action: action.to_string(),
                    ok: true,
                    output: if text.is_empty() {
                        "Already up to date.".to_string()
                    } else {
                        text
                    },
                    error: None,
                }
            } else {
                GitActionResult {
                    action: action.to_string(),
                    ok: false,
                    output: text.clone(),
                    error: Some(if text.is_empty() {
                        "git exited with an error.".to_string()
                    } else {
                        text
                    }),
                }
            }
        }
        Err(error) => GitActionResult {
            action: action.to_string(),
            ok: false,
            output: String::new(),
            error: Some(format!("Cannot run git: {error}")),
        },
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
    pub upstream: Option<String>,
}

/// Branch names are passed to git as a single argument, so refuse anything that
/// could be read as an option or that git itself considers invalid.
pub fn valid_branch_name(name: &str) -> bool {
    let name = name.trim();
    !name.is_empty()
        && name.len() <= 120
        && !name.starts_with('-')
        && !name.ends_with('/')
        && !name.ends_with(".lock")
        && !name.contains("..")
        && !name.contains("//")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
}

/// Lists local branches with their upstream, marking the checked-out one.
pub fn branches(path: &str) -> Result<Vec<GitBranch>, String> {
    if !Path::new(path.trim()).is_dir() {
        return Err(format!("Directory does not exist: {path}"));
    }
    let current = status(path).branch;
    let output = git(
        path,
        &[
            "for-each-ref",
            "--format=%(refname:short)%09%(upstream:short)",
            "refs/heads",
        ],
    )?;

    let mut list: Vec<GitBranch> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut parts = line.splitn(2, '\t');
            let name = parts.next().unwrap_or_default().trim().to_string();
            let upstream = parts
                .next()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string);
            GitBranch {
                current: current.as_deref() == Some(name.as_str()),
                name,
                upstream,
            }
        })
        .collect();
    list.sort_by(|a, b| {
        b.current
            .cmp(&a.current)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(list)
}

/// Stages every change in the working tree. Shown behind a confirmation in the UI.
pub fn stage(path: &str) -> GitActionResult {
    run_report(path, "stage", &["add", "-A"])
}

/// Commits the staged tree, optionally staging everything first.
pub fn commit(path: &str, message: &str, stage_all: bool) -> GitActionResult {
    let message = message.trim();
    if message.is_empty() {
        return failure("commit", "Write a commit message first.");
    }
    if message.chars().count() > 4000 {
        return failure("commit", "Commit messages are limited to 4000 characters.");
    }
    if stage_all {
        let staged = run_report(path, "stage", &["add", "-A"]);
        if !staged.ok {
            return GitActionResult {
                action: "commit".to_string(),
                ..staged
            };
        }
    }
    run_report(path, "commit", &["commit", "-m", message])
}

/// Creates a branch, optionally checking it out.
pub fn create_branch(path: &str, name: &str, switch: bool) -> GitActionResult {
    let name = name.trim();
    if !valid_branch_name(name) {
        return failure(
            "branch",
            "Use letters, digits and - _ . / only; no leading dash or trailing slash.",
        );
    }
    if switch {
        run_report(path, "branch", &["switch", "-c", name])
    } else {
        run_report(path, "branch", &["branch", name])
    }
}

/// Checks out an existing local branch.
pub fn switch_branch(path: &str, name: &str) -> GitActionResult {
    let name = name.trim();
    if !valid_branch_name(name) {
        return failure("branch", "That branch name is not valid.");
    }
    run_report(path, "branch", &["switch", name])
}

fn failure(action: &str, message: &str) -> GitActionResult {
    GitActionResult {
        action: action.to_string(),
        ok: false,
        output: String::new(),
        error: Some(message.to_string()),
    }
}

/// Runs git for a write operation and reports stdout/stderr as one result.
fn run_report(path: &str, action: &str, args: &[&str]) -> GitActionResult {
    if !Path::new(path.trim()).is_dir() {
        return failure(action, &format!("Directory does not exist: {path}"));
    }

    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_OPTIONAL_LOCKS", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_EDITOR", "true")
        .env("LC_ALL", "C")
        .output();

    match output {
        Ok(output) => {
            let mut text = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.trim().is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&stderr);
            }
            let text = truncate_output(text.trim());
            if output.status.success() {
                GitActionResult {
                    action: action.to_string(),
                    ok: true,
                    output: if text.is_empty() {
                        "Done.".to_string()
                    } else {
                        text
                    },
                    error: None,
                }
            } else {
                GitActionResult {
                    action: action.to_string(),
                    ok: false,
                    output: text.clone(),
                    error: Some(if text.is_empty() {
                        "git exited with an error.".to_string()
                    } else {
                        text
                    }),
                }
            }
        }
        Err(error) => failure(action, &format!("Cannot run git: {error}")),
    }
}

fn truncate_output(value: &str) -> String {
    const LIMIT: usize = 6000;
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= LIMIT {
        value.to_string()
    } else {
        let tail: String = chars[chars.len() - LIMIT..].iter().collect();
        format!("…{tail}")
    }
}

/// Clones a repository to a new directory. Refuses to touch an existing path and
/// only accepts network Git URLs.
pub fn clone(url: &str, dest: &str) -> Result<String, String> {
    let url = url.trim();
    let dest = dest.trim();
    let allowed = url.starts_with("https://")
        || url.starts_with("http://")
        || url.starts_with("ssh://")
        || url.starts_with("git@");
    if !allowed {
        return Err("Only https, ssh or git@ clone URLs are accepted.".to_string());
    }
    if dest.is_empty() {
        return Err("Choose a directory for the clone.".to_string());
    }

    let target = Path::new(dest);
    if target.exists() {
        return Err(format!("Destination already exists: {dest}"));
    }
    let parent = target
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .ok_or_else(|| "Choose a directory inside an existing folder.".to_string())?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("Cannot create {}: {e}", parent.display()))?;

    let output = Command::new("git")
        .args(["clone", "--progress", url, dest])
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "Git is not installed or not on PATH.".to_string()
            } else {
                format!("Cannot run git: {e}")
            }
        })?;

    if output.status.success() {
        Ok(dest.to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr
            .lines()
            .filter(|line| !line.trim().is_empty())
            .last()
            .unwrap_or("git clone failed")
            .trim();
        Err(detail.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_and_dirty_status() {
        let sample = "\
# branch.oid 1a2b3c4d5e6f
# branch.head main
# branch.upstream origin/main
# branch.ab +2 -1
1 .M N... 100644 100644 100644 aaa bbb src/main.rs
? notes.txt
";
        let (branch, detached, ahead, behind, changed) = parse_status(sample);
        assert_eq!(branch.as_deref(), Some("main"));
        assert!(!detached);
        assert_eq!(ahead, 2);
        assert_eq!(behind, 1);
        assert_eq!(changed, 2);
    }

    #[test]
    fn reports_detached_head_as_short_sha() {
        let sample = "# branch.oid 1234567890abcdef\n# branch.head (detached)\n";
        let (branch, detached, _, _, _) = parse_status(sample);
        assert!(detached);
        assert_eq!(branch.as_deref(), Some("1234567"));
    }

    #[test]
    fn commits_and_switches_branches_in_a_temp_repo() {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let dir = std::env::temp_dir().join(format!(
            "graft-git-test-{}-{nanos}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).expect("temp repo");
        let path = dir.to_str().expect("utf-8 path");

        let run = |args: &[&str]| {
            let output = Command::new("git")
                .arg("-C")
                .arg(path)
                .args(args)
                .env("LC_ALL", "C")
                .output()
                .expect("run git");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        };

        run(&["init", "-q"]);
        run(&["config", "user.email", "graft@example.com"]);
        run(&["config", "user.name", "GRAFT Test"]);
        std::fs::write(dir.join("README.md"), "# Test\n").expect("write file");

        let committed = commit(path, "initial commit", true);
        assert!(committed.ok, "commit failed: {:?}", committed.error);

        let initial = status(path).branch.expect("current branch");
        let listed = branches(path).expect("list branches");
        assert!(listed.iter().any(|branch| branch.current));

        let created = create_branch(path, "feat/studio", true);
        assert!(created.ok, "branch failed: {:?}", created.error);
        assert_eq!(status(path).branch.as_deref(), Some("feat/studio"));

        let switched = switch_branch(path, &initial);
        assert!(switched.ok, "switch failed: {:?}", switched.error);
        assert_eq!(status(path).branch.as_deref(), Some(initial.as_str()));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn validates_branch_names() {
        assert!(valid_branch_name("feat/profile-studio"));
        assert!(valid_branch_name("fix/git_2.1"));
        assert!(!valid_branch_name(""));
        assert!(!valid_branch_name("-force"));
        assert!(!valid_branch_name("bad name"));
        assert!(!valid_branch_name("evil..ref"));
        assert!(!valid_branch_name("trailing/"));
        assert!(!valid_branch_name("locked.lock"));
        assert!(!valid_branch_name("semi;colon"));
    }

    #[test]
    fn parses_github_remote_shapes() {
        assert_eq!(
            github_slug("git@github.com:bender/mooi.git").as_deref(),
            Some("bender/mooi")
        );
        assert_eq!(
            github_slug("https://github.com/bender/graft.git").as_deref(),
            Some("bender/graft")
        );
        assert_eq!(
            github_slug("https://github.com/bender/graft/").as_deref(),
            Some("bender/graft")
        );
        assert_eq!(
            github_slug("ssh://git@github.com:22/bender/graft.git").as_deref(),
            Some("bender/graft")
        );
        assert_eq!(github_slug("https://gitlab.com/bender/graft.git"), None);
        assert_eq!(github_slug("https://github.com/owner"), None);
        assert_eq!(github_slug(""), None);
    }
}
