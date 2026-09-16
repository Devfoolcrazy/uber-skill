//! User configuration shared by the CLI and the desktop app.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::fsutil;
use crate::targets::Target;

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
    pub library_path: Option<PathBuf>,
    #[serde(default)]
    pub recent_projects: Vec<RecentProject>,
    /// Command used to open a path in an external editor (e.g. `code`).
    #[serde(default)]
    pub editor_command: Option<String>,
}

pub fn config_path() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("UBER_SKILL_CONFIG") {
        return Some(PathBuf::from(p));
    }
    directories::ProjectDirs::from("com", "lefebvreremy", "uber-skill")
        .map(|d| d.config_dir().join("config.json"))
}

impl Config {
    pub fn load() -> Result<Config> {
        let Some(path) = config_path() else { return Ok(Config::default()) };
        if !path.is_file() {
            return Ok(Config::default());
        }
        let text = fsutil::read_to_string(&path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path().ok_or(Error::NoLibrary)?;
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        fsutil::write_string(&path, &text)
    }

    pub fn library_path(&self) -> Result<PathBuf> {
        self.library_path.clone().ok_or(Error::NoLibrary)
    }

    pub fn remember_project(&mut self, path: PathBuf, target: Target) {
        self.recent_projects.retain(|p| p.path != path);
        self.recent_projects.insert(0, RecentProject { path, target });
        self.recent_projects.truncate(10);
    }
}
