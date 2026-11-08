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
