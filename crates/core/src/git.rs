//! Opening and cloning the Git repository containing a library.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::error::{GitAction, InputError};
use crate::{Config, Error, Result};

pub mod installation;
pub mod publication;
pub mod sync;

static OPERATION_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Serialize operations that read a reviewed state and then act on it. The lock
/// guards no data, so a panic in a previous holder leaves nothing to repair.
fn exclusive() -> std::sync::MutexGuard<'static, ()> {
    OPERATION_LOCK.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn command() -> Command {
    let mut cmd = Command::new("git");
    // Do not inherit a repository selected by the launching shell.
    for key in ["GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR"] {
        cmd.env_remove(key);
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0").stdin(Stdio::null());
    if std::env::var_os("GIT_SSH_COMMAND").is_none() {
        cmd.env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes -o ConnectTimeout=15");
    }
    cmd
}

fn run(cmd: &mut Command) -> Result<String> {
    let output = cmd.output().map_err(|e| Error::GitUnavailable(e.to_string()))?;
    if !output.status.success() {
        return Err(Error::GitCommand {
            action: GitAction::Run,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim_end_matches(['\r', '\n'])
        .to_owned())
}

fn git(root: &Path) -> Command {
    let mut cmd = command();
    cmd.arg("--literal-pathspecs").arg("-C").arg(root);
    cmd.env("GIT_OPTIONAL_LOCKS", "0");
    cmd
}

fn optional(cmd: &mut Command) -> Result<Option<String>> {
    let output = cmd.output().map_err(|e| Error::io("git", e))?;
    if output.status.success() {
        Ok(Some(
            String::from_utf8_lossy(&output.stdout)
                .trim_end_matches(['\r', '\n'])
                .to_owned(),
        ))
    } else {
        Ok(None)
    }
}

fn config(root: &Path, key: &str) -> Result<Option<String>> {
    optional(git(root).args(["config", "--get", key]))
}

/// A state that only an external Git tool can resolve.
enum Interruption {
    /// A merge, rebase, cherry-pick or revert is unfinished.
    Operation,
    /// The index holds unresolved conflicts.
    Conflicts,
}

fn interruption(root: &Path) -> Result<Option<Interruption>> {
    if !run(git(root).args(["ls-files", "--unmerged", "-z"]))?.is_empty() {
        return Ok(Some(Interruption::Conflicts));
    }
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        let location = run(git(root).args(["rev-parse", "--path-format=absolute", "--git-path", marker]))?;
        if Path::new(&location).exists() {
            return Ok(Some(Interruption::Operation));
        }
    }
    Ok(None)
}

/// Require the working-tree root, including linked worktrees; reject bare repos
/// and subdirectories accidentally belonging to a parent repository.
pub fn repository_root(path: &Path) -> Result<PathBuf> {
    let path = path.canonicalize().map_err(|e| Error::io(path, e))?;
    if !path.is_dir() {
        return Err(Error::NotADirectory(path));
    }
    let root = run(command().arg("-C").arg(&path).args(["rev-parse", "--show-toplevel"]))?;
    let root = PathBuf::from(root);
    let root = root.canonicalize().map_err(|e| Error::io(&root, e))?;
    if root != path {
        return Err(Error::InvalidInput(InputError::NotRepositoryRoot(root)));
    }
    Ok(root)
}

/// Prepare configuration without saving it or mutating the previous selection.
pub fn library_config(current: &Config, path: &Path) -> Result<Config> {
    let root = repository_root(path)?;
    let mut next = current.clone();
    next.library_path = Some(root);
    next.agents_path = None;
    Ok(next)
}

/// Clone into a new directory only. No existing destination is modified.
pub fn clone_repository(url: &str, parent: &Path, name: &str) -> Result<PathBuf> {
    let url = url.trim();
    if url.is_empty() || url.starts_with('-') || url.chars().any(char::is_control) {
        return Err(Error::InvalidInput(InputError::CloneUrl));
    }
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.starts_with('.')
        || name.trim() != name
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
    {
        return Err(Error::InvalidInput(InputError::CloneName));
    }
    let parent = parent.canonicalize().map_err(|e| Error::io(parent, e))?;
    if !parent.is_dir() {
        return Err(Error::NotADirectory(parent));
    }
    let destination = parent.join(name);
    // Reserve the destination atomically, rejecting even existing empty folders
    // and dangling symlinks. On failure, never recursively delete user files.
    std::fs::create_dir(&destination).map_err(|e| {
        Error::InvalidInput(InputError::CloneDestination {
            path: destination.clone(),
            reason: e.to_string(),
        })
    })?;
    let result = run(command()
        .args([
            "-c",
            "protocol.allow=never",
            "-c",
            "protocol.https.allow=always",
            "-c",
            "protocol.http.allow=always",
            "-c",
            "protocol.ssh.allow=always",
            "-c",
            "protocol.git.allow=always",
            "-c",
            "protocol.file.allow=always",
            "clone",
            "--",
            url,
        ])
        .arg(&destination));
    if let Err(e) = result {
        let retained = std::fs::remove_dir(&destination).is_err();
        return Err(match e.during(GitAction::Clone) {
            Error::GitCommand { action, stderr } if retained => Error::GitCommand {
                action,
                stderr: format!("{stderr}\npartial directory kept: {}", destination.display()),
            },
            other => other,
        });
    }
    repository_root(&destination)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn git(path: &Path, args: &[&str]) {
        run(command().arg("-C").arg(path).args(args)).unwrap();
    }

    #[test]
    fn opens_root_and_worktree_but_rejects_subfolder_plain_and_bare() {
        let tmp = tempdir().unwrap();
        assert!(repository_root(tmp.path()).is_err());
        git(tmp.path(), &["init", "-b", "main"]);
        git(
            tmp.path(),
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "--allow-empty",
                "-m",
                "initial",
            ],
        );
        assert_eq!(repository_root(tmp.path()).unwrap(), tmp.path().canonicalize().unwrap());
        let sub = tmp.path().join("skills");
        fs::create_dir(&sub).unwrap();
        assert!(repository_root(&sub).is_err());
        let linked = tmp.path().join("linked");
        git(
            tmp.path(),
            &["worktree", "add", "-b", "linked", linked.to_str().unwrap()],
        );
        assert_eq!(repository_root(&linked).unwrap(), linked.canonicalize().unwrap());
        let bare = tmp.path().join("bare");
        git(tmp.path(), &["init", "--bare", bare.to_str().unwrap()]);
        assert!(repository_root(&bare).is_err());
    }

    #[test]
    fn clone_preserves_contents_origin_and_configuration_scope() {
        let tmp = tempdir().unwrap();
        let source = tmp.path().join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "-b", "main"]);
        fs::create_dir_all(source.join("skills/demo")).unwrap();
        fs::create_dir(source.join("agents")).unwrap();
        fs::write(
            source.join("skills/demo/SKILL.md"),
            "---\nname: demo\ndescription: Demo\n---\nInstructions\n",
        )
        .unwrap();
        fs::write(
            source.join("agents/reviewer.md"),
            "---\nname: reviewer\ndescription: Review\n---\nReview\n",
        )
        .unwrap();
        git(&source, &["add", "."]);
        git(
            &source,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "commit",
                "-m",
                "initial",
            ],
        );
        let cloned = clone_repository(source.to_str().unwrap(), tmp.path(), "clone with spaces").unwrap();
        assert_eq!(
            fs::read(cloned.join("skills/demo/SKILL.md")).unwrap(),
            fs::read(source.join("skills/demo/SKILL.md")).unwrap()
        );
        assert!(cloned.join("agents/reviewer.md").is_file());
        assert_eq!(
            run(command().arg("-C").arg(&cloned).args(["remote", "get-url", "origin"])).unwrap(),
            source.to_str().unwrap()
        );
        let old = Config {
            agents_path: Some(source.join("elsewhere")),
            editor_command: Some("code".into()),
            ..Config::default()
        };
        let next = library_config(&old, &cloned).unwrap();
        assert_eq!(next.library_path, Some(cloned));
        assert_eq!(next.agents_path, None);
        assert_eq!(next.editor_command, old.editor_command);
        assert!(old.agents_path.is_some());
    }

    #[test]
    fn clone_rejects_existing_destinations_traversal_and_failed_sources() {
        let tmp = tempdir().unwrap();
        let occupied = tmp.path().join("occupied");
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("keep"), "unchanged").unwrap();
        assert!(clone_repository("missing", tmp.path(), "occupied").is_err());
        assert_eq!(fs::read_to_string(occupied.join("keep")).unwrap(), "unchanged");
        for name in ["../escape", ".", "..", "a/b", "a\\b", ""] {
            assert!(clone_repository("missing", tmp.path(), name).is_err());
        }
        assert!(clone_repository("--help", tmp.path(), "invalid").is_err());
        assert!(!tmp.path().join("invalid").exists());
        assert!(clone_repository(tmp.path().join("missing").to_str().unwrap(), tmp.path(), "failed").is_err());
        assert!(!tmp.path().join("failed").exists());
    }
}
