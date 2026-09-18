//! Fetch the configured upstream and apply reviewed fast-forward updates only.

use super::publication::{config, git, optional};
use super::{repository_root, run, OPERATION_LOCK};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub root: String,
    pub branch: Option<String>,
    pub head: Option<String>,
    pub remote: Option<String>,
    pub remote_ref: Option<String>,
    pub upstream_ref: Option<String>,
    pub remote_head: Option<String>,
    pub ahead: usize,
    pub behind: usize,
    pub changed_files: Vec<String>,
    pub verified: bool,
    pub fetch_error: Option<String>,
    pub blocked: Option<String>,
    pub snapshot: String,
}

pub fn inspect(path: &Path) -> Result<SyncStatus> {
    let root = repository_root(path)?;
    let branch = optional(git(&root).args(["symbolic-ref", "--quiet", "--short", "HEAD"]))?;
    let head = optional(git(&root).args(["rev-parse", "--verify", "HEAD"]))?;
    let mut status = SyncStatus {
        root: root.to_string_lossy().into_owned(),
        branch,
        head,
        remote: None,
        remote_ref: None,
        upstream_ref: None,
        remote_head: None,
        ahead: 0,
        behind: 0,
        changed_files: Vec::new(),
        verified: false,
        fetch_error: None,
        blocked: None,
        snapshot: String::new(),
    };
    if let Some(branch) = &status.branch {
        status.remote = config(&root, &format!("branch.{branch}.remote"))?;
        status.remote_ref = config(&root, &format!("branch.{branch}.merge"))?;
        let reference = format!("refs/heads/{branch}");
        let tracking = run(git(&root).args(["for-each-ref", "--format=%(upstream)", &reference]))?;
        if !tracking.is_empty() {
            status.upstream_ref = Some(tracking);
        }
    } else {
        status.blocked = Some("HEAD est détachée. Choisissez une branche dans votre outil Git.".into());
    }
    if status.head.is_none() {
        status.blocked = Some("La branche locale ne contient aucun commit. Initialisez-la dans votre outil Git ou publiez un premier commit.".into());
    }
    if status.remote.as_deref().is_none_or(|r| r.is_empty() || r == ".")
        || status
            .remote_ref
            .as_deref()
            .is_none_or(|r| !r.starts_with("refs/heads/"))
        || status
            .upstream_ref
            .as_deref()
            .is_none_or(|r| !r.starts_with("refs/remotes/"))
    {
        status
            .blocked
            .get_or_insert("Aucune branche distante de suivi utilisable. Configurez-la dans votre outil Git.".into());
    }
    for marker in [
        "MERGE_HEAD",
        "CHERRY_PICK_HEAD",
        "REVERT_HEAD",
        "rebase-merge",
        "rebase-apply",
        "sequencer",
    ] {
        let location = run(git(&root).args(["rev-parse", "--path-format=absolute", "--git-path", marker]))?;
        if Path::new(&location).exists() {
            status.blocked = Some("Une opération Git est en cours. Terminez-la dans votre outil Git.".into());
        }
    }
    if !run(git(&root).args(["ls-files", "--unmerged", "-z"]))?.is_empty() {
        status.blocked = Some("Des conflits restent à résoudre dans votre outil Git.".into());
    }
    let changed = run(git(&root).args([
        "status",
        "--porcelain=v1",
        "--no-renames",
        "--untracked-files=all",
        "-z",
    ]))?;
    status.changed_files = changed
        .split('\0')
        .filter_map(|s| s.get(3..))
        .map(str::to_owned)
        .collect();
    if let Some(reference) = &status.upstream_ref {
        status.remote_head = optional(git(&root).args(["rev-parse", "--verify", &format!("{reference}^{{commit}}")]))?;
    }
    if let (Some(head), Some(upstream)) = (&status.head, &status.remote_head) {
        let counts = run(git(&root).args(["rev-list", "--left-right", "--count", &format!("{head}...{upstream}")]))?;
        let counts: Vec<usize> = counts
            .split_whitespace()
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| Error::GitCommand("État de synchronisation Git invalide.".into()))?;
        if counts.len() != 2 {
            return Err(Error::GitCommand("État de synchronisation Git incomplet.".into()));
        }
        status.ahead = counts[0];
        status.behind = counts[1];
        if status.ahead > 0 && status.behind > 0 {
            status.blocked = Some(
                "Les historiques local et distant divergent. Résolvez cette divergence dans votre outil Git.".into(),
            );
        }
    }
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&status)?);
    if let Some(remote) = &status.remote {
        hash.update(config(&root, &format!("remote.{remote}.url"))?.unwrap_or_default());
    }
    status.snapshot = hex::encode(hash.finalize());
    Ok(status)
}

/// Caller holds OPERATION_LOCK. A failed fetch is a state, not an implicit
/// permission to install or update; the UI must offer an explicit local choice.
fn check_unlocked(path: &Path) -> Result<SyncStatus> {
    let mut state = inspect(path)?;
    if let (Some(remote), Some(reference), Some(tracking)) = (&state.remote, &state.remote_ref, &state.upstream_ref) {
        if remote != "." && reference.starts_with("refs/heads/") && tracking.starts_with("refs/remotes/") {
            run(git(path).args(["check-ref-format", reference]))?;
            run(git(path).args(["check-ref-format", tracking]))?;
            // Pin source/destination instead of applying arbitrary fetch refspecs.
            match run(git(path).args([
                "fetch",
                "--no-tags",
                "--no-recurse-submodules",
                "--",
                remote,
                &format!("+{reference}:{tracking}"),
            ])) {
                Ok(_) => {
                    state = inspect(path)?;
                    state.verified = state.remote_head.is_some();
                }
                Err(e) => {
                    state.fetch_error = Some(e.to_string());
                }
            }
        }
    }
    Ok(state)
}

pub fn check(path: &Path) -> Result<SyncStatus> {
    let _guard = OPERATION_LOCK
        .lock()
        .map_err(|_| Error::GitUnavailable("Git indisponible.".into()))?;
    check_unlocked(path)
}

pub fn update(path: &Path, expected_snapshot: &str) -> Result<SyncStatus> {
    let _guard = OPERATION_LOCK
        .lock()
        .map_err(|_| Error::GitUnavailable("Git indisponible.".into()))?;
    let state = check_unlocked(path)?;
    if state.snapshot != expected_snapshot {
        return Err(Error::GitStale(
            "L’état Git a changé. Vérifiez à nouveau avant de mettre à jour.".into(),
        ));
    }
    if !state.verified {
        return Err(Error::GitBlocked(
            "La fraîcheur distante n’a pas pu être vérifiée. Aucune mise à jour appliquée.".into(),
        ));
    }
    if let Some(reason) = &state.blocked {
        return Err(Error::GitBlocked(reason.clone()));
    }
    if state.behind > 0 {
        run(git(path).args(["merge", "--ff-only", "--no-autostash", "--no-overwrite-ignore", "--no-edit", state.remote_head.as_deref().unwrap()]))
            .map_err(|e| Error::GitCommand(format!("{e}\nMise à jour interrompue. Préservez vos modifications et résolvez la situation dans votre outil Git avant de réessayer.")))?;
    }
    let mut updated = inspect(path)?;
    updated.verified = true;
    Ok(updated)
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::publication::tests::{fixture, run_git};
    use super::*;
    use std::{fs, path::PathBuf};

    pub(in crate::git) fn writer(parent: &Path, remote: &Path) -> PathBuf {
        let path = parent.join("writer");
        run_git(parent, &["clone", remote.to_str().unwrap(), path.to_str().unwrap()]);
        run_git(&path, &["config", "user.name", "Test"]);
        run_git(&path, &["config", "user.email", "test@example.com"]);
        run_git(&path, &["config", "commit.gpgSign", "false"]);
        path
    }

    pub(in crate::git) fn push_file(writer: &Path, file: &str, text: &str) {
        fs::create_dir_all(writer.join(file).parent().unwrap()).unwrap();
        fs::write(writer.join(file), text).unwrap();
        run_git(writer, &["add", "--", file]);
        run_git(writer, &["commit", "-m", "remote change"]);
        run_git(writer, &["push"]);
    }

    #[test]
    fn fetch_then_fast_forward_preserves_unrelated_staged_and_untracked_changes() {
        let (temp, local, remote) = fixture();
        let writer = writer(temp.path(), &remote);
        push_file(&writer, "a.md", "remote update\n");
        fs::write(local.join("b.md"), "staged draft\n").unwrap();
        run_git(&local, &["add", "b.md"]);
        fs::write(local.join("untracked.md"), "untracked draft\n").unwrap();
        let state = check(&local).unwrap();
        assert!(state.verified);
        assert_eq!(state.behind, 1);
        assert_eq!(fs::read_to_string(local.join("a.md")).unwrap(), "initial\n");
        let updated = update(&local, &state.snapshot).unwrap();
        assert_eq!(updated.behind, 0);
        assert_eq!(fs::read_to_string(local.join("a.md")).unwrap(), "remote update\n");
        assert_eq!(fs::read_to_string(local.join("b.md")).unwrap(), "staged draft\n");
        assert_eq!(run_git(&local, &["diff", "--cached", "--name-only"]), "b.md");
        assert_eq!(
            fs::read_to_string(local.join("untracked.md")).unwrap(),
            "untracked draft\n"
        );
        assert_eq!(
            run_git(&local, &["rev-parse", "HEAD"]),
            run_git(&remote, &["rev-parse", "main"])
        );
    }

    #[test]
    fn overlap_never_overwrites_or_autostashes_local_edits() {
        let (temp, local, remote) = fixture();
        let writer = writer(temp.path(), &remote);
        push_file(&writer, "a.md", "remote update\n");
        fs::write(local.join("a.md"), "my draft\n").unwrap();
        run_git(&local, &["config", "merge.autoStash", "true"]);
        let state = check(&local).unwrap();
        assert!(update(&local, &state.snapshot).is_err());
        assert_eq!(fs::read_to_string(local.join("a.md")).unwrap(), "my draft\n");
        assert_eq!(inspect(&local).unwrap().head, state.head);
        assert!(!local.join(".git/MERGE_HEAD").exists());
        assert!(run_git(&local, &["stash", "list"]).is_empty());
    }

    #[test]
    fn update_preserves_ignored_files_that_remote_would_replace() {
        let (temp, local, remote) = fixture();
        let writer = writer(temp.path(), &remote);
        push_file(&writer, "private.txt", "remote\n");
        fs::write(local.join(".git/info/exclude"), "private.txt\n").unwrap();
        fs::write(local.join("private.txt"), "keep local\n").unwrap();
        let state = check(&local).unwrap();
        assert!(update(&local, &state.snapshot).is_err());
        assert_eq!(fs::read_to_string(local.join("private.txt")).unwrap(), "keep local\n");
        assert_eq!(inspect(&local).unwrap().head, state.head);
    }

    #[test]
    fn divergence_requires_external_resolution() {
        let (temp, local, remote) = fixture();
        let writer = writer(temp.path(), &remote);
        push_file(&writer, "a.md", "remote\n");
        run_git(&local, &["commit", "--allow-empty", "-m", "local commit"]);
        let state = check(&local).unwrap();
        assert_eq!((state.ahead, state.behind), (1, 1));
        assert!(state.blocked.as_ref().unwrap().contains("divergent"));
        assert!(update(&local, &state.snapshot).is_err());
        assert_eq!(inspect(&local).unwrap().head, state.head);
        assert!(!local.join(".git/MERGE_HEAD").exists());
    }

    #[test]
    fn offline_deleted_branch_and_changed_remote_do_not_look_verified() {
        let (temp, local, remote) = fixture();
        let writer = writer(temp.path(), &remote);
        push_file(&writer, "a.md", "one\n");
        let reviewed = check(&local).unwrap();
        push_file(&writer, "a.md", "two\n");
        assert!(matches!(update(&local, &reviewed.snapshot), Err(Error::GitStale(_))));
        assert_eq!(fs::read_to_string(local.join("a.md")).unwrap(), "initial\n");
        run_git(
            &local,
            &[
                "remote",
                "set-url",
                "origin",
                temp.path().join("missing.git").to_str().unwrap(),
            ],
        );
        let offline = check(&local).unwrap();
        assert!(!offline.verified);
        assert!(offline.fetch_error.is_some());
        assert!(update(&local, &offline.snapshot).is_err());
        run_git(&local, &["remote", "set-url", "origin", remote.to_str().unwrap()]);
        run_git(&remote, &["update-ref", "-d", "refs/heads/main"]);
        let deleted = check(&local).unwrap();
        assert!(!deleted.verified);
        assert!(deleted.fetch_error.is_some());
    }
}
