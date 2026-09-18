//! Where each skill and agent stands against the tracked remote branch, from
//! local Git data only (no network): fast enough to decorate a whole list.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{git, optional, repository_root, run};
use crate::{Result, Skill};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemGitState {
    /// Files changed, added or deleted since the last commit.
    pub modified: bool,
    /// Commits touching the item that the tracked branch does not have yet.
    pub unpublished: bool,
    /// The tracked branch, as of the last fetch, has changes the library lacks.
    pub outdated: bool,
}

impl ItemGitState {
    pub fn is_clean(&self) -> bool {
        *self == ItemGitState::default()
    }
}

fn paths(output: &str) -> Vec<&str> {
    output.split('\0').filter(|p| !p.is_empty()).collect()
}

/// Files Git reports that an install would copy; Finder metadata and the like never count.
fn copied(path: &str) -> bool {
    !Path::new(path)
        .components()
        .any(|part| crate::fsutil::is_ignored(&part.as_os_str().to_string_lossy()))
}

/// States of the items that differ from the remote, by id. A library that is not
/// a Git repository, or a branch without upstream, simply reports less.
pub fn item_states(library_root: &Path, items: &[Skill]) -> Result<HashMap<String, ItemGitState>> {
    let Ok(root) = repository_root(library_root) else {
        return Ok(HashMap::new());
    };
    let status = run(git(&root).args([
        "status",
        "--porcelain=v1",
        "--no-renames",
        "--untracked-files=all",
        "-z",
    ]))?;
    let modified: Vec<&str> = paths(&status)
        .into_iter()
        .filter_map(|record| record.get(3..))
        .filter(|p| copied(p))
        .collect();

    let (mut ahead, mut behind) = (String::new(), String::new());
    let upstream = optional(git(&root).args(["rev-parse", "--verify", "--quiet", "@{upstream}"]))?;
    let head = optional(git(&root).args(["rev-parse", "--verify", "--quiet", "HEAD"]))?;
    if let (Some(upstream), Some(head)) = (&upstream, &head) {
        if let Some(base) = optional(git(&root).args(["merge-base", head, upstream]))? {
            let changed = |to: &str| run(git(&root).args(["diff", "--name-only", "--no-renames", "-z", &base, to]));
            ahead = changed(head)?;
            behind = changed(upstream)?;
        }
    }
    let (ahead, behind) = (paths(&ahead), paths(&behind));

    let mut states = HashMap::new();
    for item in items {
        let Ok(relative) = item.path.strip_prefix(&root) else {
            continue;
        };
        let touches = |files: &[&str]| files.iter().any(|f| Path::new(f).starts_with(relative));
        let state = ItemGitState {
            modified: touches(&modified),
            unpublished: touches(&ahead),
            outdated: touches(&behind),
        };
        if !state.is_clean() {
            states.insert(item.id.clone(), state);
        }
    }
    Ok(states)
}

#[cfg(test)]
mod tests {
    use super::super::publication::tests::{fixture, run_git};
    use super::super::sync::tests::{push_file, writer};
    use super::*;
    use crate::Library;
    use std::fs;

    #[test]
    fn tells_local_edits_unpushed_commits_and_remote_changes_apart() {
        let (temp, local, remote) = fixture();
        fs::create_dir(local.join("skills")).unwrap();
        let lib = Library::open(local.join("skills")).unwrap();
        for id in ["edited", "committed", "behind", "clean"] {
            lib.create(id, "A skill", None, &[], &[]).unwrap();
        }
        run_git(&local, &["add", "."]);
        run_git(&local, &["commit", "-m", "seed"]);
        run_git(&local, &["push"]);
        let items = || lib.scan().unwrap().skills;
        assert!(item_states(&local, &items()).unwrap().is_empty());

        fs::write(local.join("skills/edited/notes.md"), "draft\n").unwrap();
        fs::write(local.join("skills/clean/.DS_Store"), "finder").unwrap();
        fs::write(local.join("skills/committed/extra.md"), "more\n").unwrap();
        run_git(&local, &["add", "skills/committed"]);
        run_git(&local, &["commit", "-m", "local only"]);
        let other = writer(temp.path(), &remote);
        push_file(
            &other,
            "skills/behind/SKILL.md",
            "---\nname: behind\ndescription: Newer\n---\nRemote\n",
        );

        // Remote changes are only known after a fetch.
        let before = item_states(&local, &items()).unwrap();
        assert!(!before.contains_key("behind"));
        run_git(&local, &["fetch"]);
        let states = item_states(&local, &items()).unwrap();
        assert_eq!(
            states["edited"],
            ItemGitState {
                modified: true,
                ..Default::default()
            }
        );
        assert_eq!(
            states["committed"],
            ItemGitState {
                unpublished: true,
                ..Default::default()
            }
        );
        assert_eq!(
            states["behind"],
            ItemGitState {
                outdated: true,
                ..Default::default()
            }
        );
        assert!(!states.contains_key("clean"));
    }

    #[test]
    fn a_plain_folder_or_a_branch_without_upstream_is_not_an_error() {
        let plain = tempfile::tempdir().unwrap();
        let lib = Library::open(plain.path()).unwrap();
        lib.create("one", "A skill", None, &[], &[]).unwrap();
        assert!(item_states(plain.path(), &lib.scan().unwrap().skills)
            .unwrap()
            .is_empty());

        let (_temp, local, _) = fixture();
        run_git(&local, &["branch", "--unset-upstream"]);
        fs::create_dir(local.join("skills")).unwrap();
        let lib = Library::open(local.join("skills")).unwrap();
        lib.create("one", "A skill", None, &[], &[]).unwrap();
        let states = item_states(&local, &lib.scan().unwrap().skills).unwrap();
        assert_eq!(
            states["one"],
            ItemGitState {
                modified: true,
                ..Default::default()
            }
        );
    }
}
