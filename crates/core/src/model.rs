use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// What kind of library item this is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ItemKind {
    /// A directory containing `SKILL.md` (plus optional support files).
    #[default]
    Skill,
    /// A single markdown file with frontmatter (`<name>.md`), e.g. Claude Code subagents.
    Agent,
}

impl ItemKind {
    pub fn label(&self) -> &'static str {
        match self {
            ItemKind::Skill => "skill",
            ItemKind::Agent => "agent",
        }
    }

    pub fn parse(s: &str) -> Option<ItemKind> {
        match s.to_ascii_lowercase().as_str() {
            "skill" | "skills" => Some(ItemKind::Skill),
            "agent" | "agents" => Some(ItemKind::Agent),
            _ => None,
        }
    }
}

/// A library item (skill or agent) as indexed from the library.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Skill {
    #[serde(default)]
    pub kind: ItemKind,
    /// Directory name (skill) or file stem (agent); the stable identifier used for install/lock records.
    pub id: String,
    /// `name` from the frontmatter (should equal `id`).
    pub name: String,
    pub description: String,
    pub category: Option<String>,
    pub tags: Vec<String>,
    /// Host tools this skill depends on (empty = works everywhere).
    pub hosts: Vec<String>,
    /// Absolute path to the skill directory, or to the agent's `.md` file.
    pub path: PathBuf,
    /// Path relative to the library root (may be nested).
    pub rel_path: PathBuf,
    /// Content hash of the whole directory.
    pub hash: String,
    /// Files in the skill directory, relative, sorted.
    pub files: Vec<String>,
    /// Other frontmatter keys as strings (e.g. `allowed-tools`, `argument-hint`).
    pub extra: BTreeMap<String, String>,
    /// Length of the markdown body in characters.
    pub body_chars: usize,
    /// Last modification time of SKILL.md (RFC 3339), if available.
    pub modified_at: Option<String>,
}

/// A problem found while scanning the library (non-fatal).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScanWarning {
    pub path: PathBuf,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ScanResult {
    pub skills: Vec<Skill>,
    pub warnings: Vec<ScanWarning>,
}
