use std::path::Path;
use std::process::Command;

/// Snapshot of the Git status within the password store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatus {
    pub is_git_repo: bool,
    pub branch: String,
    pub remote_name: Option<String>,
    pub remote_url: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub is_dirty: bool,
    pub recent_commits: Vec<String>,
}

impl GitStatus {
    pub fn not_repo() -> Self {
        Self {
            is_git_repo: false,
            branch: String::new(),
            remote_name: None,
            remote_url: None,
            ahead: 0,
            behind: 0,
            is_dirty: false,
            recent_commits: Vec::new(),
        }
    }
}

/// Checks if ~/.password-store/.git exists.
pub fn is_git_repo(store_dir: &Path) -> bool {
    store_dir.join(".git").exists()
}

/// Initializes a git repository in the password store using `pass git init`.
pub fn pass_git_init() -> Result<(), String> {
    let output = Command::new("pass")
        .args(["git", "init"])
        .output()
        .map_err(|e| format!("Failed to run `pass git init`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("pass git init failed: {}", stderr.trim()));
    }

    Ok(())
}

/// Fetches full Git status for the password store.
pub fn get_git_status(store_dir: &Path) -> GitStatus {
    if !is_git_repo(store_dir) {
        return GitStatus::not_repo();
    }

    // 1. Query status -b --porcelain
    let (branch, _upstream, ahead, behind, is_dirty) = match Command::new("pass")
        .args(["git", "status", "-b", "--porcelain"])
        .output()
    {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            parse_git_status_porcelain(&s)
        }
        _ => ("unknown".to_string(), None, 0, 0, false),
    };

    // 2. Query remotes
    let (remote_name, remote_url) =
        match Command::new("pass").args(["git", "remote", "-v"]).output() {
            Ok(out) if out.status.success() => {
                let s = String::from_utf8_lossy(&out.stdout);
                parse_git_remote(&s)
            }
            _ => (None, None),
        };

    // 3. Query recent commits (last 5)
    let recent_commits = match Command::new("pass")
        .args(["git", "log", "-n", "5", "--oneline"])
        .output()
    {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8_lossy(&out.stdout);
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(|l| l.to_string())
                .collect()
        }
        _ => Vec::new(),
    };

    GitStatus {
        is_git_repo: true,
        branch,
        remote_name,
        remote_url,
        ahead,
        behind,
        is_dirty,
        recent_commits,
    }
}

/// Parses output from `git status -b --porcelain`.
/// Returns (branch, upstream, ahead, behind, is_dirty).
pub fn parse_git_status_porcelain(output: &str) -> (String, Option<String>, usize, usize, bool) {
    let mut lines = output.lines();
    let first_line = lines.next().unwrap_or("").trim();

    let mut branch = "main".to_string();
    let mut upstream = None;
    let mut ahead = 0;
    let mut behind = 0;

    if let Some(rest) = first_line.strip_prefix("##") {
        let rest = rest.trim();
        // Check for "No commits yet on <branch>"
        if let Some(b) = rest.strip_prefix("No commits yet on ") {
            branch = b.trim().to_string();
        } else {
            // e.g. "main...origin/main [ahead 1, behind 2]" or "main"
            let (branch_part, meta_part) = match rest.split_once(' ') {
                Some((b, m)) => (b.trim(), Some(m.trim())),
                None => (rest, None),
            };

            if let Some((local, up)) = branch_part.split_once("...") {
                branch = local.to_string();
                upstream = Some(up.to_string());
            } else {
                branch = branch_part.to_string();
            }

            if let Some(meta) = meta_part {
                // Parse bracketed info e.g. "[ahead 1, behind 2]"
                let stripped = meta.trim_start_matches('[').trim_end_matches(']');
                for part in stripped.split(',') {
                    let part = part.trim();
                    if let Some(num_str) = part.strip_prefix("ahead ") {
                        ahead = num_str.parse().unwrap_or(0);
                    } else if let Some(num_str) = part.strip_prefix("behind ") {
                        behind = num_str.parse().unwrap_or(0);
                    }
                }
            }
        }
    }

    let is_dirty = lines.any(|l| !l.trim().is_empty());

    (branch, upstream, ahead, behind, is_dirty)
}

/// Parses output from `git remote -v`.
/// Returns (remote_name, remote_url).
pub fn parse_git_remote(output: &str) -> (Option<String>, Option<String>) {
    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 {
            let name = parts[0].to_string();
            let url = parts[1].to_string();
            return (Some(name), Some(url));
        }
    }
    (None, None)
}

/// Sets or updates the `origin` remote URL.
pub fn pass_git_set_remote(url: &str) -> Result<(), String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return Err("Remote URL cannot be empty".to_string());
    }

    // Check if origin already exists
    let output = Command::new("pass")
        .args(["git", "remote"])
        .output()
        .map_err(|e| format!("Failed to check remotes: {e}"))?;

    let remotes = String::from_utf8_lossy(&output.stdout);
    let has_origin = remotes.lines().any(|l| l.trim() == "origin");

    let status = if has_origin {
        Command::new("pass")
            .args(["git", "remote", "set-url", "origin", trimmed])
            .output()
    } else {
        Command::new("pass")
            .args(["git", "remote", "add", "origin", trimmed])
            .output()
    };

    let res = status.map_err(|e| format!("Failed to set remote: {e}"))?;
    if !res.status.success() {
        let err = String::from_utf8_lossy(&res.stderr);
        return Err(format!("Failed to set remote: {}", err.trim()));
    }

    Ok(())
}

/// Pushes local encrypted commits to remote.
pub fn pass_git_push(branch: &str) -> Result<String, String> {
    // Try standard push first
    let output = Command::new("pass")
        .args(["git", "push"])
        .output()
        .map_err(|e| format!("Failed to run `pass git push`: {e}"))?;

    if output.status.success() {
        let msg = String::from_utf8_lossy(&output.stdout);
        let err = String::from_utf8_lossy(&output.stderr);
        let out = if !msg.trim().is_empty() {
            msg.trim().to_string()
        } else if !err.trim().is_empty() {
            err.trim().to_string()
        } else {
            "Pushed successfully".to_string()
        };
        return Ok(out);
    }

    // Check if failure is due to missing upstream
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("no upstream branch") || stderr.contains("set-upstream") {
        let target_branch = if branch.is_empty() { "main" } else { branch };
        let upstream_res = Command::new("pass")
            .args(["git", "push", "-u", "origin", target_branch])
            .output()
            .map_err(|e| format!("Failed to run `pass git push -u`: {e}"))?;

        if upstream_res.status.success() {
            return Ok(format!("Pushed and set upstream to origin/{target_branch}"));
        } else {
            let err2 = String::from_utf8_lossy(&upstream_res.stderr);
            return Err(format!("Git push failed: {}", err2.trim()));
        }
    }

    Err(format!("Git push failed: {}", stderr.trim()))
}

/// Pulls latest encrypted changes from remote.
pub fn pass_git_pull() -> Result<String, String> {
    let output = Command::new("pass")
        .args(["git", "pull"])
        .output()
        .map_err(|e| format!("Failed to run `pass git pull`: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("Git pull failed: {}", stderr.trim()));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let msg = stdout.trim();
    if msg.is_empty() {
        Ok("Already up to date".to_string())
    } else {
        Ok(msg.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_git_status_porcelain_synced() {
        let sample = "## main...origin/main\n";
        let (branch, upstream, ahead, behind, is_dirty) = parse_git_status_porcelain(sample);
        assert_eq!(branch, "main");
        assert_eq!(upstream.as_deref(), Some("origin/main"));
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
        assert!(!is_dirty);
    }

    #[test]
    fn test_parse_git_status_porcelain_ahead_behind() {
        let sample = "## feature/sync...origin/feature/sync [ahead 3, behind 1]\n M test.gpg\n";
        let (branch, upstream, ahead, behind, is_dirty) = parse_git_status_porcelain(sample);
        assert_eq!(branch, "feature/sync");
        assert_eq!(upstream.as_deref(), Some("origin/feature/sync"));
        assert_eq!(ahead, 3);
        assert_eq!(behind, 1);
        assert!(is_dirty);
    }

    #[test]
    fn test_parse_git_status_porcelain_no_remote() {
        let sample = "## main\n";
        let (branch, upstream, ahead, behind, is_dirty) = parse_git_status_porcelain(sample);
        assert_eq!(branch, "main");
        assert_eq!(upstream, None);
        assert_eq!(ahead, 0);
        assert_eq!(behind, 0);
        assert!(!is_dirty);
    }

    #[test]
    fn test_parse_git_remote() {
        let sample = "origin\tgit@github.com:user/passwords.git (fetch)\norigin\tgit@github.com:user/passwords.git (push)\n";
        let (name, url) = parse_git_remote(sample);
        assert_eq!(name.as_deref(), Some("origin"));
        assert_eq!(url.as_deref(), Some("git@github.com:user/passwords.git"));
    }
}
