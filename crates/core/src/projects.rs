//! The projects the user follows, and what each of them has installed from the
//! library, across every install target that holds a lock file. This is what
//! tells where an edited skill is now out of date.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::install::{self, DriftState, InstalledSkill, LockFile};
use crate::library::Library;
use crate::model::ItemKind;
use crate::targets::Target;
use crate::Result;

/// What one target of a project holds for one kind of item.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TargetInstall {
    pub target: Target,
    pub kind: ItemKind,
    pub items: Vec<InstalledSkill>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProjectOverview {
    pub path: PathBuf,
    pub name: String,
    /// False when the folder is gone: the project stays listed until the user removes it.
    pub exists: bool,
    pub installs: Vec<TargetInstall>,
    /// Copies that are simply behind the library: safe to update in bulk.
    pub behind: usize,
}

/// Targets worth scanning in `project`: the built-in ones, plus any custom folder
/// this project was used with.
fn candidate_targets(cfg: &Config, project: &Path) -> Vec<Target> {
    let mut targets: Vec<Target> = Target::ALL.to_vec();
    for recent in cfg.recent_projects.iter().filter(|r| r.path == project) {
        if matches!(recent.target, Target::Custom(_)) && !targets.contains(&recent.target) {
            targets.push(recent.target.clone());
        }
    }
    targets
}

fn overview_of(cfg: &Config, libraries: &[Library], project: &Path) -> Result<ProjectOverview> {
    let name = project
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| project.display().to_string());
    let mut overview = ProjectOverview {
        path: project.to_path_buf(),
        name,
        exists: project.is_dir(),
        installs: Vec::new(),
        behind: 0,
    };
    if !overview.exists {
        return Ok(overview);
    }
    for target in candidate_targets(cfg, project) {
        for kind in [ItemKind::Skill, ItemKind::Agent] {
            let Ok(dir) = target.dir_for(kind, project) else {
                continue;
            };
            // Only what the app installed: a folder without a lock is not ours to report.
            if !dir.join(install::LOCK_FILE).is_file() || LockFile::load(&dir)?.skills.is_empty() {
                continue;
            }
            let library = libraries.iter().find(|l| l.kind() == kind);
            let items = install::status(library, project, &target, kind)?;
            overview.behind += items.iter().filter(|i| i.state == DriftState::LibraryUpdated).count();
            overview.installs.push(TargetInstall {
                target: target.clone(),
                kind,
                items,
            });
        }
    }
    Ok(overview)
}

/// Every followed project with its installed items, in the order they are followed.
pub fn overview(cfg: &Config) -> Result<Vec<ProjectOverview>> {
    let libraries: Vec<Library> = [ItemKind::Skill, ItemKind::Agent]
        .into_iter()
        .filter_map(|kind| cfg.path_for(kind).ok().and_then(|p| Library::open_kind(p, kind).ok()))
        .collect();
    cfg.tracked_projects
        .iter()
        .map(|project| overview_of(cfg, &libraries, project))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn library() -> (tempfile::TempDir, Config) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("lib/skills")).unwrap();
        fs::create_dir_all(root.join("lib/agents")).unwrap();
        let skills = Library::open(root.join("lib/skills")).unwrap();
        skills.create("one", "First skill", None, &[], &[]).unwrap();
        skills.create("two", "Second skill", None, &[], &[]).unwrap();
        Library::open_kind(root.join("lib/agents"), ItemKind::Agent)
            .unwrap()
            .create("reviewer", "Reviews", None, &[], &[])
            .unwrap();
        (
            tmp,
            Config {
                library_path: Some(root.join("lib")),
                ..Config::default()
            },
        )
    }

    #[test]
    fn reports_every_target_with_a_lock_and_counts_copies_behind() {
        let (tmp, mut cfg) = library();
        let root = tmp.path().canonicalize().unwrap();
        let skills = Library::open(cfg.skills_path().unwrap()).unwrap();
        let agents = Library::open_kind(cfg.agents_path().unwrap(), ItemKind::Agent).unwrap();
        let (game, site, gone) = (root.join("game"), root.join("site"), root.join("gone"));
        for project in [&game, &site] {
            fs::create_dir(project).unwrap();
        }
        install::install(&skills.get("one").unwrap(), &game, &Target::ClaudeCode).unwrap();
        install::install(&skills.get("two").unwrap(), &game, &Target::Cursor).unwrap();
        install::install(&agents.get("reviewer").unwrap(), &game, &Target::ClaudeCode).unwrap();
        install::install(&skills.get("one").unwrap(), &site, &Target::ClaudeCode).unwrap();
        // A hand-made skills folder, never touched by the app, is not reported.
        fs::create_dir_all(site.join(".cursor/skills/handmade")).unwrap();
        fs::write(
            site.join(".cursor/skills/handmade/SKILL.md"),
            "---\nname: handmade\ndescription: x\n---\nBody\n",
        )
        .unwrap();
        for project in [&game, &site, &gone] {
            cfg.track_project(project);
        }
        cfg.track_project(&game);
        assert_eq!(cfg.tracked_projects.len(), 3);

        skills
            .write_file("one", "notes.md", "changed in the library\n")
            .unwrap();
        let projects = overview(&cfg).unwrap();

        let game = &projects[0];
        assert_eq!((game.name.as_str(), game.exists, game.behind), ("game", true, 1));
        let found: Vec<(String, ItemKind, Vec<&str>)> = game
            .installs
            .iter()
            .map(|i| {
                (
                    i.target.label(),
                    i.kind,
                    i.items.iter().map(|s| s.id.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                (Target::ClaudeCode.label(), ItemKind::Skill, vec!["one"]),
                (Target::ClaudeCode.label(), ItemKind::Agent, vec!["reviewer"]),
                (Target::Cursor.label(), ItemKind::Skill, vec!["two"]),
            ]
        );
        assert_eq!(game.installs[0].items[0].state, DriftState::LibraryUpdated);
        assert_eq!(game.installs[2].items[0].state, DriftState::UpToDate);

        assert_eq!((projects[1].behind, projects[1].installs.len()), (1, 1));
        assert_eq!((projects[2].exists, projects[2].installs.len()), (false, 0));

        cfg.untrack_project(&gone);
        assert_eq!(overview(&cfg).unwrap().len(), 2);
    }
}
