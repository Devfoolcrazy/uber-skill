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
    /// The user's own folders (`~/.claude`, `~/.agents`): items there are available in every project.
    pub global: bool,
    pub installs: Vec<TargetInstall>,
    /// Copies that are simply behind the library: safe to update in bulk.
    pub behind: usize,
}

/// Harnesses whose per-user folders the app manages: Claude Code and Codex, the
/// ones actually in use here. `~/.claude/skills`, `~/.claude/agents`, `~/.agents/skills`.
pub const GLOBAL_TARGETS: [Target; 2] = [Target::ClaudeCode, Target::Agents];

/// The root of the global install: the user's home folder.
pub fn global_root() -> Option<PathBuf> {
    directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf())
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

fn overview_of(cfg: &Config, libraries: &[Library], project: &Path, global: bool) -> Result<ProjectOverview> {
    let name = if global {
        "Global".to_string()
    } else {
        project
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| project.display().to_string())
    };
    let mut overview = ProjectOverview {
        path: project.to_path_buf(),
        name,
        exists: project.is_dir(),
        global,
        installs: Vec::new(),
        behind: 0,
    };
    if !overview.exists {
        return Ok(overview);
    }
    let targets = if global {
        GLOBAL_TARGETS.to_vec()
    } else {
        candidate_targets(cfg, project)
    };
    for target in targets {
        for kind in [ItemKind::Skill, ItemKind::Agent] {
            let Ok(dir) = target.dir_for(kind, project) else {
                continue;
            };
            // In a project, only what the app installed: a folder without a lock is not ours
            // to report. The global folders are shown whole, so hand-made items can be adopted.
            let managed = dir.join(install::LOCK_FILE).is_file() && !LockFile::load(&dir)?.skills.is_empty();
            if !managed && !(global && dir.is_dir()) {
                continue;
            }
            let library = libraries.iter().find(|l| l.kind() == kind);
            let items = install::status(library, project, &target, kind)?;
            if items.is_empty() {
                continue;
            }
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

/// The global install first, then every followed project in the order they are followed.
pub fn overview(cfg: &Config) -> Result<Vec<ProjectOverview>> {
    overview_with_home(cfg, global_root().as_deref())
}

pub fn overview_with_home(cfg: &Config, home: Option<&Path>) -> Result<Vec<ProjectOverview>> {
    let libraries: Vec<Library> = [ItemKind::Skill, ItemKind::Agent]
        .into_iter()
        .filter_map(|kind| cfg.path_for(kind).ok().and_then(|p| Library::open_kind(p, kind).ok()))
        .collect();
    let global = home.map(|h| overview_of(cfg, &libraries, h, true)).transpose()?;
    let projects = cfg
        .tracked_projects
        .iter()
        .filter(|p| Some(p.as_path()) != home)
        .map(|p| overview_of(cfg, &libraries, p, false));
    global.into_iter().map(Ok).chain(projects).collect()
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
        let projects = overview_with_home(&cfg, None).unwrap();

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
        assert_eq!(overview_with_home(&cfg, None).unwrap().len(), 2);
    }

    #[test]
    fn the_home_folder_comes_first_whole_and_never_as_a_tracked_project() {
        let (tmp, mut cfg) = library();
        let home = tmp.path().canonicalize().unwrap().join("home");
        let skills = Library::open(cfg.skills_path().unwrap()).unwrap();
        install::install(&skills.get("one").unwrap(), &home, &Target::ClaudeCode).unwrap();
        // Hand-made and Codex items, never installed by the app, are listed as untracked.
        fs::create_dir_all(home.join(".claude/skills/handmade")).unwrap();
        fs::write(
            home.join(".claude/skills/handmade/SKILL.md"),
            "---\nname: handmade\ndescription: x\n---\nBody\n",
        )
        .unwrap();
        fs::create_dir_all(home.join(".agents/skills/two")).unwrap();
        fs::write(
            home.join(".agents/skills/two/SKILL.md"),
            "---\nname: two\ndescription: x\n---\nBody\n",
        )
        .unwrap();
        // Cursor is not a global harness here.
        fs::create_dir_all(home.join(".cursor/skills/ignored")).unwrap();
        fs::write(
            home.join(".cursor/skills/ignored/SKILL.md"),
            "---\nname: ignored\ndescription: x\n---\nBody\n",
        )
        .unwrap();
        cfg.track_project(&home);

        let projects = overview_with_home(&cfg, Some(&home)).unwrap();
        assert_eq!(projects.len(), 1);
        let global = &projects[0];
        assert!(global.global);
        assert_eq!(global.name, "Global");
        let found: Vec<(String, Vec<(&str, DriftState)>)> = global
            .installs
            .iter()
            .map(|i| {
                (
                    i.target.label(),
                    i.items.iter().map(|s| (s.id.as_str(), s.state)).collect(),
                )
            })
            .collect();
        assert_eq!(
            found,
            [
                (
                    Target::ClaudeCode.label(),
                    vec![("handmade", DriftState::Untracked), ("one", DriftState::UpToDate)]
                ),
                (Target::Agents.label(), vec![("two", DriftState::Untracked)]),
            ]
        );
    }
}
