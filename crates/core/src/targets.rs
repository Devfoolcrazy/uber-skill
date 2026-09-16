//! Where items get installed inside a project, per host tool.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};
use crate::model::ItemKind;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "path")]
pub enum Target {
    /// `.claude/skills` and `.claude/agents` (Claude Code)
    ClaudeCode,
    /// `.agents/skills` (Codex, Amp, Copilot CLI and the `skills` CLI convention)
    Agents,
    /// `.cursor/skills`
    Cursor,
    /// `.github/skills` (GitHub Copilot)
    Copilot,
    /// Any directory relative to the project root.
    Custom(PathBuf),
}

impl Target {
    pub const ALL: [Target; 4] = [Target::ClaudeCode, Target::Agents, Target::Cursor, Target::Copilot];

    /// Directory (relative to the project root) for items of `kind`, if this host supports them.
    pub fn rel_dir_for(&self, kind: ItemKind) -> Option<PathBuf> {
        match (self, kind) {
            (Target::Custom(p), _) => Some(p.clone()),
            (Target::ClaudeCode, ItemKind::Skill) => Some(PathBuf::from(".claude/skills")),
            (Target::ClaudeCode, ItemKind::Agent) => Some(PathBuf::from(".claude/agents")),
            (Target::Agents, ItemKind::Skill) => Some(PathBuf::from(".agents/skills")),
            (Target::Cursor, ItemKind::Skill) => Some(PathBuf::from(".cursor/skills")),
            (Target::Copilot, ItemKind::Skill) => Some(PathBuf::from(".github/skills")),
            (_, ItemKind::Agent) => None,
        }
    }

    pub fn dir_for(&self, kind: ItemKind, project_root: &Path) -> Result<PathBuf> {
        self.rel_dir_for(kind)
            .map(|rel| project_root.join(rel))
            .ok_or_else(|| Error::Unsupported(format!("{} does not support {}s", self.label(), kind.label())))
    }

    pub fn supports(&self, kind: ItemKind) -> bool {
        self.rel_dir_for(kind).is_some()
    }

    pub fn label(&self) -> String {
        match self {
            Target::ClaudeCode => "Claude Code".into(),
            Target::Agents => "Agents (Codex, Amp, Copilot CLI)".into(),
            Target::Cursor => "Cursor".into(),
            Target::Copilot => "GitHub Copilot".into(),
            Target::Custom(p) => format!("Custom ({})", p.display()),
        }
    }

    /// Whether a `hosts` entry (e.g. `claude-code`, `codex`, `cursor`) is a known host name.
    pub fn is_known_host(host: &str) -> bool {
        !matches!(Target::parse(host), Target::Custom(_))
    }

    /// Whether a skill declaring `hosts` can be installed on this target.
    /// Custom targets accept everything; an empty `hosts` list means "works everywhere".
    pub fn accepts_hosts(&self, hosts: &[String]) -> bool {
        if hosts.is_empty() || matches!(self, Target::Custom(_)) {
            return true;
        }
        hosts.iter().any(|h| &Target::parse(h) == self)
    }

    pub fn parse(s: &str) -> Target {
        match s.to_ascii_lowercase().as_str() {
            "claude" | "claude-code" | "claudecode" => Target::ClaudeCode,
            "agents" | "codex" | "amp" | "copilot-cli" => Target::Agents,
            "cursor" => Target::Cursor,
            "copilot" | "github" | "github-copilot" => Target::Copilot,
            other => Target::Custom(PathBuf::from(other)),
        }
    }
}
