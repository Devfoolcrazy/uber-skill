//! User configuration shared by the CLI and the desktop app.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::fsutil;
use crate::targets::Target;

/// Conventional layout under the library root: `skills/` and `agents/`.
pub const SKILLS_SUBDIR: &str = "skills";
pub const AGENTS_SUBDIR: &str = "agents";
/// Legacy agents folder (before the `skills/` + `agents/` layout).
pub const LEGACY_AGENTS_SUBDIR: &str = "_AGENTS";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RecentProject {
    pub path: PathBuf,
    #[serde(default = "default_target")]
    pub target: Target,
}

fn default_target() -> Target {
    Target::ClaudeCode
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Config {
    /// Library root. Skills live in `<root>/skills` (or at the root itself, legacy layout).
    pub library_path: Option<PathBuf>,
    /// Explicit agents folder. Defaults to `<root>/agents` (or `<root>/_AGENTS`, legacy).
    #[serde(default)]
    pub agents_path: Option<PathBuf>,
    #[serde(default)]
    pub recent_projects: Vec<RecentProject>,
    /// Command used to open a path in an external editor (e.g. `code`).
    #[serde(default)]
    pub editor_command: Option<String>,
    /// Projects the user follows across the library, oldest first. Unlike
    /// `recent_projects`, nothing drops out of this list by itself.
    #[serde(default)]
    pub tracked_projects: Vec<PathBuf>,
}

pub fn config_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("UBER_SKILL_CONFIG") {
        return Some(PathBuf::from(p));
    }
    directories::ProjectDirs::from("com", "lefebvreremy", "uber-skill").map(|d| d.config_dir().join("config.json"))
}

impl Config {
    pub fn load() -> Result<Config> {
        let Some(path) = config_path() else {
            return Ok(Config::default());
        };
        if !path.is_file() {
            return Ok(Config::default());
        }
        let text = fsutil::read_to_string(&path)?;
        let mut cfg: Config = serde_json::from_str(&text)?;
        // Configurations written before projects were followed: start from the recent ones.
        if cfg.tracked_projects.is_empty() {
            let recent: Vec<PathBuf> = cfg.recent_projects.iter().rev().map(|r| r.path.clone()).collect();
            for project in &recent {
                cfg.track_project(project);
            }
        }
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path().ok_or(Error::NoLibrary)?;
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        fsutil::write_string(&path, &text)
    }

    /// The configured library root.
    pub fn library_path(&self) -> Result<PathBuf> {
        self.library_path.clone().ok_or(Error::NoLibrary)
    }

    /// Where skills live: `<root>/skills` if it exists, else the root itself.
    pub fn skills_path(&self) -> Result<PathBuf> {
        let root = self.library_path()?;
        let sub = root.join(SKILLS_SUBDIR);
        Ok(if sub.is_dir() { sub } else { root })
    }

    /// Where agents live: the explicit `agents_path`, else `<root>/agents`,
    /// else the legacy `<root>/_AGENTS` if it exists.
    pub fn agents_path(&self) -> Result<PathBuf> {
        if let Some(p) = &self.agents_path {
            return Ok(p.clone());
        }
        let root = self.library_path()?;
        let modern = root.join(AGENTS_SUBDIR);
        let legacy = root.join(LEGACY_AGENTS_SUBDIR);
        Ok(if !modern.is_dir() && legacy.is_dir() {
            legacy
        } else {
            modern
        })
    }

    pub fn path_for(&self, kind: crate::model::ItemKind) -> Result<PathBuf> {
        match kind {
            crate::model::ItemKind::Skill => self.skills_path(),
            crate::model::ItemKind::Agent => self.agents_path(),
        }
    }

    /// Follow a project. Returns false when it already was.
    pub fn track_project(&mut self, path: &Path) -> bool {
        let known = self.tracked_projects.iter().any(|p| p == path);
        if !known {
            self.tracked_projects.push(path.to_path_buf());
        }
        !known
    }

    pub fn untrack_project(&mut self, path: &Path) {
        self.tracked_projects.retain(|p| p != path);
    }

    pub fn remember_project(&mut self, path: PathBuf, target: Target) {
        self.recent_projects.retain(|p| p.path != path);
        self.recent_projects.insert(0, RecentProject { path, target });
        self.recent_projects.truncate(10);
    }
}
