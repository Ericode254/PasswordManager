use crate::pass::commands::{self, DecryptedEntry};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Clone, Debug)]
pub struct Revision {
    pub commit: String,
    pub date: String,
    pub path: String,
}

pub struct Preview {
    pub entry: DecryptedEntry,
    pub revision: Revision,
    current: Option<Vec<u8>>,
    head: Vec<u8>,
}

#[derive(Default)]
pub struct HistoryView {
    pub revisions: Vec<Revision>,
    pub selected: usize,
    pub preview: Option<Preview>,
    pub confirming: bool,
    pub error: Option<String>,
}

fn git(store: &Path, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(store)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Cannot run Git: {e}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "History requires a Git password store with commits: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

pub fn list(store: &Path, path: Option<&str>) -> Result<Vec<Revision>, String> {
    if !crate::pass::git::is_git_repo(store) {
        return Err(
            "History needs Git. Press G, then i to initialize the store repository.".into(),
        );
    }
    let mut args = vec![
        "log",
        "--format=%x1e%H%x00%cs%x00",
        "--name-only",
        "-z",
        "--diff-filter=AM",
        "--no-renames",
        "--max-count=100",
        "--",
    ];
    let literal = path.map(|path| format!(":(literal){path}.gpg"));
    args.push(literal.as_deref().unwrap_or("*.gpg"));
    let output = git(store, &args)?;
    Ok(parse_log(&String::from_utf8_lossy(&output)))
}

fn parse_log(log: &str) -> Vec<Revision> {
    let mut revisions = Vec::new();
    for record in log.split('\u{1e}').skip(1) {
        let mut fields = record.split('\0');
        let commit = fields.next().unwrap_or("");
        let date = fields.next().unwrap_or("");
        if !valid_commit(commit) {
            continue;
        }
        for file in fields {
            let Some(path) = file.trim_start_matches('\n').strip_suffix(".gpg") else {
                continue;
            };
            if crate::editor::valid_path(path) {
                revisions.push(Revision {
                    commit: commit.into(),
                    date: date.into(),
                    path: path.into(),
                });
            }
        }
    }
    revisions
}

fn valid_commit(commit: &str) -> bool {
    matches!(commit.len(), 40 | 64) && commit.bytes().all(|c| c.is_ascii_hexdigit())
}

fn target(store: &Path, path: &str) -> Result<PathBuf, String> {
    if !crate::editor::valid_path(path) {
        return Err("Invalid entry path".into());
    }
    let relative = format!("{path}.gpg");
    let mut target = store.to_path_buf();
    for component in Path::new(&relative).components() {
        target.push(component);
        match fs::symlink_metadata(&target) {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err("Cannot restore through a symbolic link".into());
            }
            Ok(_) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(target)
}

fn current(store: &Path, path: &str) -> Result<Option<Vec<u8>>, String> {
    match fs::read(target(store, path)?) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn preview(store: &Path, revision: Revision) -> Result<Preview, String> {
    if !valid_commit(&revision.commit) {
        return Err("Invalid revision".into());
    }
    let current = current(store, &revision.path)?;
    let head = git(store, &["rev-parse", "HEAD"])?;
    let blob = git(
        store,
        &[
            "show",
            &format!("{}:{}.gpg", revision.commit, revision.path),
        ],
    )?;
    let mut child = Command::new("gpg")
        .arg("--decrypt")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Cannot run GPG: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("GPG input unavailable")?;
    let writer = std::thread::spawn(move || stdin.write_all(&blob));
    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let written = writer.join().map_err(|_| "GPG input worker stopped")?;
    if !output.status.success() {
        return Err(format!(
            "Cannot decrypt this revision: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    written.map_err(|e| e.to_string())?;
    let bytes = zeroize::Zeroizing::new(output.stdout);
    let content = std::str::from_utf8(&bytes).map_err(|_| "Entry is not UTF-8 text")?;
    Ok(Preview {
        entry: commands::parse_entry(content),
        revision,
        current,
        head,
    })
}

fn check_restore(store: &Path, preview: &Preview) -> Result<(), String> {
    if git(store, &["rev-parse", "HEAD"])? != preview.head
        || current(store, &preview.revision.path)? != preview.current
    {
        return Err(
            "The store changed since preview. Preview the revision again before restoring.".into(),
        );
    }
    let path = format!(":(literal){}.gpg", preview.revision.path);
    if !git(
        store,
        &[
            "status",
            "--porcelain",
            "--untracked-files=all",
            "--",
            &path,
        ],
    )?
    .is_empty()
    {
        return Err(
            "This entry has uncommitted changes. Commit or resolve them before restoring.".into(),
        );
    }
    Ok(())
}

pub fn restore(store: &Path, preview: Preview) -> Result<(), String> {
    check_restore(store, &preview)?;
    // Re-encrypt for today's recipients and let pass create a normal new commit.
    commands::pass_insert_in(
        store,
        &preview.revision.path,
        &preview.entry.content,
        preview.current.is_some(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lists_deleted_versions_and_refuses_stale_or_dirty_restore() {
        let dir = std::env::temp_dir().join(format!("passtui-history-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        git(&dir, &["init"]).unwrap();
        let commit = || {
            git(&dir, &["add", "--all"]).unwrap();
            git(
                &dir,
                &[
                    "-c",
                    "user.name=Test",
                    "-c",
                    "user.email=test@example.invalid",
                    "commit",
                    "-m",
                    "test",
                ],
            )
            .unwrap();
        };
        fs::write(dir.join("login.gpg"), "encrypted version one").unwrap();
        commit();
        fs::write(dir.join("login.gpg"), "encrypted version two").unwrap();
        commit();
        let revisions = list(&dir, Some("login")).unwrap();
        assert_eq!(revisions.len(), 2);
        let preview = Preview {
            entry: commands::parse_entry("test"),
            revision: revisions[1].clone(),
            current: current(&dir, "login").unwrap(),
            head: git(&dir, &["rev-parse", "HEAD"]).unwrap(),
        };
        assert!(check_restore(&dir, &preview).is_ok());
        fs::write(dir.join("login.gpg"), "external edit").unwrap();
        assert!(
            check_restore(&dir, &preview)
                .unwrap_err()
                .contains("changed")
        );
        let dirty = Preview {
            current: current(&dir, "login").unwrap(),
            ..preview
        };
        assert!(
            check_restore(&dir, &dirty)
                .unwrap_err()
                .contains("uncommitted")
        );
        fs::remove_file(dir.join("login.gpg")).unwrap();
        commit();
        assert_eq!(list(&dir, None).unwrap().len(), 2);
        assert!(list(&dir, Some("log*")).unwrap().is_empty());
        assert!(target(&dir, "../outside").is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("/tmp", dir.join("link")).unwrap();
            assert!(target(&dir, "link/outside").is_err());
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
