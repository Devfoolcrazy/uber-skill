//! Review working-tree contents, commit selected files, and push the current branch.

use std::collections::BTreeSet;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{config, exclusive, git, interruption, optional, repository_root, run, Interruption};
use crate::error::{BlockReason, ErrorPayload, GitAction, InputError, Stale};
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChangedFile {
    pub path: String,
    pub status: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preview {
    pub root: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub remote: Option<String>,
    pub remote_branch: Option<String>,
    pub pending_count: usize,
    pub pending_commits: Vec<String>,
    pub files: Vec<ChangedFile>,
    pub snapshot: String,
    pub blocked: Option<BlockReason>,
}

#[derive(Debug, Serialize)]
pub struct PublicationResult {
    /// Present only if this attempt created a commit.
    pub commit: Option<String>,
    pub pushed: bool,
    pub push_error: Option<ErrorPayload>,
}

pub fn preview(path: &Path) -> Result<Preview> {
    let root = repository_root(path)?;
    let branch = optional(git(&root).args(["symbolic-ref", "--quiet", "--short", "HEAD"]))?;
    let head = optional(git(&root).args(["rev-parse", "--verify", "HEAD"]))?;
    let (remote, remote_branch) = match &branch {
        Some(b) => (
            config(&root, &format!("branch.{b}.remote"))?,
            config(&root, &format!("branch.{b}.merge"))?.and_then(|r| r.strip_prefix("refs/heads/").map(str::to_owned)),
        ),
        None => (None, None),
    };
    // Later checks take precedence: the last reason found is the one reported.
    let mut blocked = if branch.is_none() {
        Some(BlockReason::DetachedHead)
    } else if remote.as_deref().is_none_or(|r| r.is_empty() || r == ".") || remote_branch.is_none() {
        Some(BlockReason::NoUpstream)
    } else {
        None
    };
    let mut push_url = String::new();
    if let Some(r) = remote.as_deref().filter(|r| *r != ".") {
        match single_push_url(&root, r)? {
            Some(url) => push_url = url,
            None => blocked = Some(BlockReason::AmbiguousPushUrl),
        }
    }
    match interruption(&root)? {
        Some(Interruption::Operation) => blocked = Some(BlockReason::OperationInProgress),
        Some(Interruption::Conflicts) => blocked = Some(BlockReason::Conflicts),
        None => {}
    }
    let (pending_count, pending_commits) = pending(&root, head.as_deref())?;
    let changes = working_tree_changes(&root, head.as_deref())?;
    if changes.touches_submodule {
        blocked = Some(BlockReason::SubmoduleChange);
    }
    let mut preview = Preview {
        root: root.to_string_lossy().into_owned(),
        branch,
        head,
        remote,
        remote_branch,
        pending_count,
        pending_commits,
        files: changes.files,
        snapshot: String::new(),
        blocked,
    };
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&preview)?);
    hash.update(changes.tree.as_bytes());
    hash.update(push_url.as_bytes());
    preview.snapshot = hex::encode(hash.finalize());
    Ok(preview)
}

/// The push URL of `remote`, or None when it is missing or fans out to several.
fn single_push_url(root: &Path, remote: &str) -> Result<Option<String>> {
    let urls = optional(git(root).args(["remote", "get-url", "--push", "--all", remote]))?;
    Ok(urls.filter(|u| u.lines().count() == 1))
}

/// Local commits not yet on the upstream: their count and the first 20 subjects.
fn pending(root: &Path, head: Option<&str>) -> Result<(usize, Vec<String>)> {
    let Some(head) = head else { return Ok((0, Vec::new())) };
    let upstream = optional(git(root).args(["rev-parse", "--verify", "@{upstream}"]))?;
    let range = upstream
        .map(|u| format!("{u}..{head}"))
        .unwrap_or_else(|| head.to_owned());
    let count = run(git(root).args(["rev-list", "--count", &range]))?
        .parse()
        .map_err(|_| Error::GitCommand {
            action: GitAction::Parse,
            stderr: "rev-list --count".into(),
        })?;
    let commits = run(git(root).args(["log", "--format=%h %s", "-20", &range, "--"]))?
        .lines()
        .map(str::to_owned)
        .collect();
    Ok((count, commits))
}

struct WorkingTreeChanges {
    files: Vec<ChangedFile>,
    /// Tree of the whole working tree, so the snapshot covers every byte shown.
    tree: String,
    touches_submodule: bool,
}

/// Diff the working tree against HEAD through an isolated index: Git renders
/// additions, deletions, binaries, modes and clean filters without disturbing
/// any user staging in the real index.
fn working_tree_changes(root: &Path, head: Option<&str>) -> Result<WorkingTreeChanges> {
    let temp = tempfile::tempdir().map_err(|e| Error::io("temporary index", e))?;
    let index = temp.path().join("index");
    let temp_git = || {
        let mut cmd = git(root);
        cmd.env("GIT_INDEX_FILE", &index);
        cmd
    };
    run(temp_git().args(["read-tree", head.unwrap_or("--empty")]))?;
    run(temp_git().args(["add", "--all", "--", "."]))?;
    let tree = run(temp_git().arg("write-tree"))?;
    let names = run(temp_git().args(["diff", "--cached", "--name-status", "--no-renames", "-z"]))?;
    let parts: Vec<_> = names.split('\0').filter(|s| !s.is_empty()).collect();
    if parts.len() % 2 != 0 {
        return Err(Error::GitCommand {
            action: GitAction::Parse,
            stderr: "diff --name-status".into(),
        });
    }
    let mut files = Vec::new();
    let mut touches_submodule = false;
    for pair in parts.chunks_exact(2) {
        let (status, path) = (pair[0], pair[1]);
        let diff = run(temp_git().args([
            "diff",
            "--cached",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-renames",
            "--",
            path,
        ]))?;
        // 160000 is the gitlink mode; confirm on the raw record, not on file text.
        if diff.contains("160000")
            && run(temp_git().args(["diff", "--cached", "--raw", "--", path]))?.contains("160000")
        {
            touches_submodule = true;
        }
        files.push(ChangedFile {
            path: path.to_owned(),
            status: status.to_owned(),
            diff,
        });
    }
    Ok(WorkingTreeChanges {
        files,
        tree,
        touches_submodule,
    })
}

/// Commit only selected files, preserving staging for all unselected paths.
/// Existing local commits are included in the explicit, non-forced branch push.
pub fn publish(path: &Path, snapshot: &str, paths: &[String], message: &str) -> Result<PublicationResult> {
    let _guard = exclusive();
    let current = preview(path)?;
    if current.snapshot != snapshot {
        return Err(Error::GitStale(Stale::Preview));
    }
    if let Some(reason) = current.blocked {
        return Err(Error::GitBlocked(reason));
    }
    let selected: BTreeSet<_> = paths.iter().collect();
    if selected.len() != paths.len() || selected.iter().any(|p| !current.files.iter().any(|f| &f.path == *p)) {
        return Err(Error::InvalidInput(InputError::UnknownSelection));
    }
    if !paths.is_empty() && message.trim().is_empty() {
        return Err(Error::InvalidInput(InputError::EmptyCommitMessage));
    }
    if paths.is_empty() && current.pending_count == 0 {
        return Err(Error::InvalidInput(InputError::NothingToPublish));
    }
    let root = Path::new(&current.root);
    let mut commit = None;
    if !paths.is_empty() {
        run(git(root).args(["add", "--all", "--"]).args(paths))?;
        // --only explicitly excludes other staged files. Hooks and signing stay
        // enabled according to the user's Git configuration.
        run(git(root)
            .args(["commit", "--only", "--cleanup=verbatim", "-m", message, "--"])
            .args(paths))
        .map_err(|e| e.during(GitAction::Commit))?;
        commit = Some(run(git(root).args(["rev-parse", "HEAD"]))?);
    }
    let head = commit
        .as_ref()
        .or(current.head.as_ref())
        .ok_or(Error::InvalidInput(InputError::NothingToPublish))?;
    let remote = current.remote.as_deref().unwrap();
    let remote_branch = current.remote_branch.as_deref().unwrap();
    // Pin the source OID: an external commit made while pushing is not included.
    // Ignore broad mirror/follow-tags settings, never force or publish tags.
    let push = run(git(root).args([
        "-c",
        &format!("remote.{remote}.mirror=false"),
        "push",
        "--no-follow-tags",
        "--recurse-submodules=no",
        "--",
        remote,
        &format!("{head}:refs/heads/{remote_branch}"),
    ]));
    Ok(PublicationResult {
        commit,
        pushed: push.is_ok(),
        push_error: push.err().map(ErrorPayload::from),
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::{tempdir, TempDir};

    pub(in crate::git) fn run_git(root: &Path, args: &[&str]) -> String {
        run(git(root).args(args)).unwrap()
    }

    pub(in crate::git) fn fixture() -> (TempDir, PathBuf, PathBuf) {
        let temp = tempdir().unwrap();
        let local = temp.path().join("local");
        let remote = temp.path().join("remote.git");
        fs::create_dir(&local).unwrap();
        run_git(temp.path(), &["init", "--bare", "-b", "main", remote.to_str().unwrap()]);
        run_git(&local, &["init", "-b", "main"]);
        run_git(&local, &["config", "user.name", "Test"]);
        run_git(&local, &["config", "user.email", "test@example.com"]);
        run_git(&local, &["config", "commit.gpgSign", "false"]);
        for file in ["a.md", "b.md", "delete.md"] {
            fs::write(local.join(file), "initial\n").unwrap();
        }
        run_git(&local, &["add", "."]);
        run_git(&local, &["commit", "-m", "initial"]);
        run_git(&local, &["remote", "add", "origin", remote.to_str().unwrap()]);
        run_git(&local, &["push", "-u", "origin", "main"]);
        (temp, local, remote)
    }

    #[test]
    fn selected_files_only_preserving_other_staging_and_preview_index() {
        let (_temp, local, remote) = fixture();
        fs::write(local.join("a.md"), "published\n").unwrap();
        fs::write(local.join("b.md"), "draft\n").unwrap();
        run_git(&local, &["add", "b.md"]);
        fs::remove_file(local.join("delete.md")).unwrap();
        let special = ":[note] with space\nand newline.md";
        fs::write(local.join(special), "new file\n").unwrap();
        fs::write(local.join(".gitignore"), "ignored.txt\n").unwrap();
        fs::write(local.join("ignored.txt"), "ignored\n").unwrap();
        let index_before = fs::read(local.join(".git/index")).unwrap();
        let p = preview(&local).unwrap();
        assert_eq!(fs::read(local.join(".git/index")).unwrap(), index_before);
        assert!(!p.files.iter().any(|f| f.path == "ignored.txt"));
        assert!(p
            .files
            .iter()
            .any(|f| f.path == special && f.diff.contains("+new file")));
        let result = publish(
            &local,
            &p.snapshot,
            &["a.md".into(), "delete.md".into(), special.into()],
            "Selected changes",
        )
        .unwrap();
        assert!(result.pushed, "{:?}", result.push_error);
        assert_eq!(run_git(&remote, &["show", "main:a.md"]), "published");
        assert_eq!(run_git(&remote, &["show", "main:b.md"]), "initial");
        assert_eq!(run_git(&local, &["diff", "--cached", "--name-only"]), "b.md");
        assert!(run_git(&local, &["diff", "--cached", "--", "b.md"]).contains("+draft"));
        assert!(!run_git(&remote, &["ls-tree", "--name-only", "main"]).contains("delete.md"));
        assert_eq!(fs::read_to_string(local.join("b.md")).unwrap(), "draft\n");
    }

    #[test]
    fn rejects_stale_preview_and_unreviewed_paths_before_committing() {
        let (_temp, local, _) = fixture();
        fs::write(local.join("a.md"), "version one\n").unwrap();
        let p = preview(&local).unwrap();
        let head = p.head.clone();
        fs::write(local.join("a.md"), "version two\n").unwrap();
        assert!(matches!(
            publish(&local, &p.snapshot, &["a.md".into()], "stale"),
            Err(Error::GitStale(_))
        ));
        let p = preview(&local).unwrap();
        for paths in [
            vec!["../escape".into()],
            vec!["*".into()],
            vec!["a.md".into(), "a.md".into()],
        ] {
            assert!(publish(&local, &p.snapshot, &paths, "invalid").is_err());
        }
        assert!(publish(&local, &p.snapshot, &["a.md".into()], "  ").is_err());
        assert_eq!(preview(&local).unwrap().head, head);
        assert!(run_git(&local, &["diff", "--cached", "--name-only"]).is_empty());
    }

    #[test]
    fn failed_push_keeps_commit_and_retry_does_not_create_another() {
        let (temp, local, remote) = fixture();
        run_git(
            &local,
            &[
                "remote",
                "set-url",
                "--push",
                "origin",
                temp.path().join("missing.git").to_str().unwrap(),
            ],
        );
        fs::write(local.join("a.md"), "pending\n").unwrap();
        let p = preview(&local).unwrap();
        let result = publish(&local, &p.snapshot, &["a.md".into()], "pending").unwrap();
        assert!(!result.pushed);
        assert!(result.commit.is_some());
        let head = run_git(&local, &["rev-parse", "HEAD"]);
        assert_eq!(run_git(&remote, &["show", "main:a.md"]), "initial");
        run_git(
            &local,
            &["remote", "set-url", "--push", "origin", remote.to_str().unwrap()],
        );
        let p = preview(&local).unwrap();
        assert_eq!(p.pending_count, 1);
        assert!(p.files.is_empty());
        let retry = publish(&local, &p.snapshot, &[], "").unwrap();
        assert!(retry.pushed);
        assert!(retry.commit.is_none());
        assert_eq!(run_git(&local, &["rev-parse", "HEAD"]), head);
        assert_eq!(run_git(&remote, &["rev-parse", "main"]), head);
        assert_eq!(preview(&local).unwrap().pending_count, 0);
    }

    #[test]
    fn distant_advance_rejects_push_without_force() {
        let (temp, local, remote) = fixture();
        let other = temp.path().join("other");
        run_git(
            temp.path(),
            &["clone", remote.to_str().unwrap(), other.to_str().unwrap()],
        );
        fs::write(other.join("remote-only.md"), "remote\n").unwrap();
        run_git(&other, &["add", "."]);
        run_git(
            &other,
            &[
                "-c",
                "user.name=Test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgSign=false",
                "commit",
                "-m",
                "remote advance",
            ],
        );
        run_git(&other, &["push"]);
        let remote_head = run_git(&remote, &["rev-parse", "main"]);
        fs::write(local.join("a.md"), "local\n").unwrap();
        let p = preview(&local).unwrap();
        let result = publish(&local, &p.snapshot, &["a.md".into()], "local").unwrap();
        assert!(!result.pushed);
        assert!(result.commit.is_some());
        assert_eq!(run_git(&remote, &["rev-parse", "main"]), remote_head);
    }

    #[test]
    fn first_commit_in_empty_clone_can_select_only_one_file() {
        let temp = tempdir().unwrap();
        let remote = temp.path().join("empty.git");
        run_git(temp.path(), &["init", "--bare", "-b", "main", remote.to_str().unwrap()]);
        let local = super::super::clone_repository(remote.to_str().unwrap(), temp.path(), "local").unwrap();
        run_git(&local, &["config", "user.name", "Test"]);
        run_git(&local, &["config", "user.email", "test@example.com"]);
        run_git(&local, &["config", "commit.gpgSign", "false"]);
        fs::write(local.join("first.md"), "first\n").unwrap();
        fs::write(local.join("draft.md"), "draft\n").unwrap();
        run_git(&local, &["add", "draft.md"]);
        let p = preview(&local).unwrap();
        assert!(p.head.is_none());
        assert!(p.blocked.is_none(), "{:?}", p.blocked);
        let result = publish(&local, &p.snapshot, &["first.md".into()], "initial selection").unwrap();
        assert!(result.pushed, "{:?}", result.push_error);
        assert_eq!(run_git(&remote, &["ls-tree", "--name-only", "main"]), "first.md");
        assert_eq!(run_git(&local, &["diff", "--cached", "--name-only"]), "draft.md");
    }

    #[test]
    fn blocks_detached_missing_tracking_and_git_operations() {
        let (_temp, local, _) = fixture();
        run_git(&local, &["checkout", "--detach"]);
        assert_eq!(preview(&local).unwrap().blocked, Some(BlockReason::DetachedHead));
        run_git(&local, &["checkout", "main"]);
        fs::write(local.join(".git/MERGE_HEAD"), run_git(&local, &["rev-parse", "HEAD"])).unwrap();
        let p = preview(&local).unwrap();
        assert!(p.blocked.is_some());
        assert!(publish(&local, &p.snapshot, &[], "").is_err());
        fs::remove_file(local.join(".git/MERGE_HEAD")).unwrap();
        run_git(&local, &["branch", "--unset-upstream"]);
        assert_eq!(preview(&local).unwrap().blocked, Some(BlockReason::NoUpstream));
    }

    #[test]
    fn honors_tracked_branch_destination_instead_of_local_branch_name() {
        let (_temp, local, remote) = fixture();
        let original = run_git(&remote, &["rev-parse", "main"]);
        run_git(&local, &["config", "branch.main.merge", "refs/heads/release"]);
        fs::write(local.join("a.md"), "release\n").unwrap();
        let p = preview(&local).unwrap();
        let result = publish(&local, &p.snapshot, &["a.md".into()], "release").unwrap();
        assert!(result.pushed);
        assert_eq!(run_git(&remote, &["rev-parse", "main"]), original);
        assert_eq!(run_git(&remote, &["show", "release:a.md"]), "release");
    }

    #[cfg(unix)]
    #[test]
    fn rejected_commit_hook_preserves_files_and_does_not_push() {
        use std::os::unix::fs::PermissionsExt;
        let (temp, local, remote) = fixture();
        let hooks = temp.path().join("hooks");
        fs::create_dir(&hooks).unwrap();
        let hook = hooks.join("pre-commit");
        fs::write(&hook, "#!/bin/sh\necho 'Commit rejected by test hook' >&2\nexit 1\n").unwrap();
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
        run_git(&local, &["config", "core.hooksPath", hooks.to_str().unwrap()]);
        fs::write(local.join("a.md"), "local edit\n").unwrap();
        let p = preview(&local).unwrap();
        let error = publish(&local, &p.snapshot, &["a.md".into()], "rejected")
            .unwrap_err()
            .to_string();
        assert!(error.contains("Commit rejected by test hook"));
        assert_eq!(preview(&local).unwrap().head, p.head);
        assert_eq!(run_git(&remote, &["show", "main:a.md"]), "initial");
        assert_eq!(fs::read_to_string(local.join("a.md")).unwrap(), "local edit\n");
        assert_eq!(run_git(&local, &["diff", "--cached", "--name-only"]), "a.md");
    }

    #[test]
    fn binary_additions_and_renames_are_reviewable_and_publishable() {
        let (_temp, local, remote) = fixture();
        fs::rename(local.join("a.md"), local.join("renamed.md")).unwrap();
        let binary = [0, 1, 2, 0xff, 0, 5];
        fs::write(local.join("asset.bin"), binary).unwrap();
        let p = preview(&local).unwrap();
        assert!(p
            .files
            .iter()
            .any(|f| f.path == "asset.bin" && f.diff.contains("Binary files")));
        assert!(p.files.iter().any(|f| f.path == "a.md" && f.status == "D"));
        assert!(p.files.iter().any(|f| f.path == "renamed.md" && f.status == "A"));
        let result = publish(
            &local,
            &p.snapshot,
            &["a.md".into(), "renamed.md".into(), "asset.bin".into()],
            "rename and binary",
        )
        .unwrap();
        assert!(result.pushed);
        assert_eq!(run_git(&remote, &["show", "main:renamed.md"]), "initial");
        assert_eq!(
            git(&remote).args(["show", "main:asset.bin"]).output().unwrap().stdout,
            binary
        );
    }
}
