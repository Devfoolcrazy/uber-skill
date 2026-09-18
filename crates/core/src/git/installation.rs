//! Prepare a reviewed installation and reject source changes before copying.

use super::publication::git;
use super::{run, sync, OPERATION_LOCK};
use crate::install::{self, SourceState};
use crate::{Config, Error, ItemKind, Library, LockEntry, Result, Skill, Target};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreparedItem {
    pub id: String,
    pub source: std::path::PathBuf,
    pub hash: String,
    pub source_state: SourceState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallationPlan {
    pub root: String,
    pub kind: ItemKind,
    pub target: Target,
    pub items: Vec<PreparedItem>,
    pub git: Option<sync::SyncStatus>,
    pub git_error: Option<String>,
    pub warnings: Vec<String>,
}

fn source_state(skill: &Skill, status: Option<&sync::SyncStatus>) -> Result<SourceState> {
    let Some(status) = status else {
        return Ok(SourceState::Unverified);
    };
    let root = Path::new(&status.root);
    let Ok(relative) = skill.path.strip_prefix(root) else {
        return Ok(SourceState::Unverified);
    };
    let dirty = run(git(root)
        .args([
            "status",
            "--porcelain=v1",
            "--no-renames",
            "--untracked-files=all",
            "--ignored",
            "-z",
            "--",
        ])
        .arg(relative))?;
    let has_copied_changes = dirty.split('\0').filter_map(|record| record.get(3..)).any(|path| {
        !Path::new(path)
            .components()
            .any(|part| crate::fsutil::is_ignored(&part.as_os_str().to_string_lossy()))
    });
    if has_copied_changes {
        return Ok(SourceState::LocalDraft);
    }
    if let (Some(head), Some(upstream)) = (&status.head, &status.remote_head) {
        let commits = run(git(root)
            .args(["log", "-1", "--format=%H", &format!("{upstream}..{head}"), "--"])
            .arg(relative))?;
        if !commits.is_empty() {
            return Ok(SourceState::UnpublishedCommit);
        }
    }
    Ok(if status.verified {
        SourceState::Published
    } else {
        SourceState::Unverified
    })
}

pub fn prepare(cfg: &Config, kind: ItemKind, ids: &[String], target: &Target) -> Result<InstallationPlan> {
    if ids.is_empty() {
        return Err(Error::InvalidInput("Aucun élément sélectionné.".into()));
    }
    if !target.supports(kind) {
        return Err(Error::Unsupported("Cette cible ne gère pas ce type d’élément.".into()));
    }
    let root = cfg.library_path()?;
    let (git, git_error) = match sync::check(&root) {
        Ok(status) => (Some(status), None),
        Err(e) => (None, Some(e.to_string())),
    };
    let lib = Library::open_kind(cfg.path_for(kind)?, kind)?;
    let mut items = Vec::new();
    let mut warnings = Vec::new();
    for id in ids {
        let skill = lib.get(id)?;
        if let Some(warning) = install::host_mismatch(&skill, target) {
            warnings.push(warning);
        }
        let state = source_state(&skill, git.as_ref())?;
        items.push(PreparedItem {
            id: id.clone(),
            source: skill.path,
            hash: skill.hash,
            source_state: state,
        });
    }
    Ok(InstallationPlan {
        root: root.to_string_lossy().into_owned(),
        kind,
        target: target.clone(),
        items,
        git,
        git_error,
        warnings,
    })
}

pub fn install_prepared(cfg: &Config, plan: &InstallationPlan, project: &Path) -> Result<Vec<LockEntry>> {
    let _guard = OPERATION_LOCK
        .lock()
        .map_err(|_| Error::GitUnavailable("Git indisponible.".into()))?;
    if cfg.library_path()?.to_string_lossy() != plan.root {
        return Err(Error::GitStale(
            "La bibliothèque a changé. Recommencez la vérification.".into(),
        ));
    }
    if let Some(expected) = &plan.git {
        if sync::inspect(Path::new(&plan.root))?.snapshot != expected.snapshot {
            return Err(Error::GitStale(
                "L’état Git a changé depuis la vérification. Actualisez avant d’installer.".into(),
            ));
        }
    }
    let lib = Library::open_kind(cfg.path_for(plan.kind)?, plan.kind)?;
    // Validate the entire batch before writing anything into the project.
    let mut skills = Vec::new();
    for expected in &plan.items {
        let skill = lib.get(&expected.id)?;
        if skill.hash != expected.hash || skill.path != expected.source {
            return Err(Error::GitStale(format!(
                "{} a changé depuis la vérification. Actualisez avant d’installer.",
                skill.id
            )));
        }
        skills.push(skill);
    }
    skills
        .iter()
        .zip(&plan.items)
        .map(|(skill, item)| install::install_with_state(skill, project, &plan.target, Some(item.source_state)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::publication::tests::{fixture, run_git};
    use super::super::sync::tests::{push_file, writer};
    use super::*;
    use std::fs;

    fn seed(local: &Path) -> Config {
        fs::create_dir(local.join("skills")).unwrap();
        fs::create_dir(local.join("agents")).unwrap();
        let lib = Library::open(local.join("skills")).unwrap();
        lib.create("one", "One", None, &[], &[]).unwrap();
        lib.create("two", "Two", None, &[], &[]).unwrap();
        Library::open_kind(local.join("agents"), ItemKind::Agent)
            .unwrap()
            .create("reviewer", "Review", None, &[], &[])
            .unwrap();
        run_git(local, &["add", "."]);
        run_git(local, &["commit", "-m", "seed items"]);
        run_git(local, &["push"]);
        Config {
            library_path: Some(local.to_path_buf()),
            ..Config::default()
        }
    }

    #[test]
    fn installs_explicit_local_draft_offline_and_persists_source_state() {
        let (temp, local, _) = fixture();
        let cfg = seed(&local);
        run_git(
            &local,
            &[
                "remote",
                "set-url",
                "origin",
                temp.path().join("unavailable.git").to_str().unwrap(),
            ],
        );
        fs::write(local.join("skills/one/draft.txt"), "local draft\n").unwrap();
        let plan = prepare(
            &cfg,
            ItemKind::Skill,
            &["one".into(), "two".into()],
            &Target::ClaudeCode,
        )
        .unwrap();
        assert!(!plan.git.as_ref().unwrap().verified);
        assert_eq!(plan.items[0].source_state, SourceState::LocalDraft);
        assert_eq!(plan.items[1].source_state, SourceState::Unverified);
        let project = temp.path().join("project");
        let entries = install_prepared(&cfg, &plan, &project).unwrap();
        assert_eq!(entries[0].source_state, Some(SourceState::LocalDraft));
        assert_eq!(
            fs::read_to_string(project.join(".claude/skills/one/draft.txt")).unwrap(),
            "local draft\n"
        );
        let lock = install::LockFile::load(&project.join(".claude/skills")).unwrap();
        assert_eq!(lock.skills["one"].source_state, Some(SourceState::LocalDraft));
    }

    #[test]
    fn updates_then_reprepares_and_installs_new_content_for_skills_and_agents() {
        let (temp, local, remote) = fixture();
        let cfg = seed(&local);
        let writer = writer(temp.path(), &remote);
        push_file(&writer, "skills/one/instructions.txt", "new remote instructions\n");
        push_file(
            &writer,
            "agents/reviewer.md",
            "---\nname: reviewer\ndescription: New reviewer\n---\nNew instructions\n",
        );
        let old = prepare(&cfg, ItemKind::Skill, &["one".into()], &Target::ClaudeCode).unwrap();
        assert_eq!(old.git.as_ref().unwrap().behind, 2);
        sync::update(&local, &old.git.as_ref().unwrap().snapshot).unwrap();
        let project = temp.path().join("project");
        assert!(install_prepared(&cfg, &old, &project).is_err());
        assert!(!project.exists());
        let plan = prepare(&cfg, ItemKind::Skill, &["one".into()], &Target::ClaudeCode).unwrap();
        assert_eq!(plan.items[0].source_state, SourceState::Published);
        install_prepared(&cfg, &plan, &project).unwrap();
        assert_eq!(
            fs::read_to_string(project.join(".claude/skills/one/instructions.txt")).unwrap(),
            "new remote instructions\n"
        );
        let plan = prepare(&cfg, ItemKind::Agent, &["reviewer".into()], &Target::ClaudeCode).unwrap();
        install_prepared(&cfg, &plan, &project).unwrap();
        assert!(fs::read_to_string(project.join(".claude/agents/reviewer.md"))
            .unwrap()
            .contains("New instructions"));
    }

    #[test]
    fn rejects_changed_batch_before_copying_and_recognizes_unpublished_commits() {
        let (temp, local, _) = fixture();
        let cfg = seed(&local);
        fs::write(local.join("skills/one/new.txt"), "not yet pushed\n").unwrap();
        run_git(&local, &["add", "."]);
        run_git(&local, &["commit", "-m", "unpublished"]);
        let plan = prepare(
            &cfg,
            ItemKind::Skill,
            &["one".into(), "two".into()],
            &Target::ClaudeCode,
        )
        .unwrap();
        assert_eq!(plan.items[0].source_state, SourceState::UnpublishedCommit);
        assert_eq!(plan.items[1].source_state, SourceState::Published);
        fs::write(local.join("skills/two/changed.txt"), "after review\n").unwrap();
        let project = temp.path().join("project");
        assert!(install_prepared(&cfg, &plan, &project).is_err());
        assert!(!project.exists());
        let old: LockEntry =
            serde_json::from_str(r#"{"id":"one","source":"/old/one","hash":"hash","installed_at":"date"}"#).unwrap();
        assert_eq!(old.source_state, None);
    }

    #[test]
    fn source_state_ignores_files_excluded_from_copy_but_flags_copied_ignored_files() {
        let (_temp, local, _) = fixture();
        let cfg = seed(&local);
        fs::write(local.join("skills/one/.DS_Store"), "finder metadata").unwrap();
        fs::write(local.join(".git/info/exclude"), "private.txt\n.DS_Store\n").unwrap();
        let plan = prepare(&cfg, ItemKind::Skill, &["one".into()], &Target::ClaudeCode).unwrap();
        assert_eq!(plan.items[0].source_state, SourceState::Published);
        fs::write(local.join("skills/one/private.txt"), "copied local content").unwrap();
        let plan = prepare(&cfg, ItemKind::Skill, &["one".into()], &Target::ClaudeCode).unwrap();
        assert_eq!(plan.items[0].source_state, SourceState::LocalDraft);
    }
}
