//! Where skills get installed inside a project, per host tool.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "path")]
pub enum Target {
    /// `.claude/skills` (Claude Code)
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

    pub fn rel_dir(&self) -> PathBuf {
        match self {
            Target::ClaudeCode => PathBuf::from(".claude/skills"),
            Target::Agents => PathBuf::from(".agents/skills"),
            Target::Cursor => PathBuf::from(".cursor/skills"),
            Target::Copilot => PathBuf::from(".github/skills"),
            Target::Custom(p) => p.clone(),
        }
    }

    pub fn dir(&self, project_root: &Path) -> PathBuf {
        project_root.join(self.rel_dir())
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
            "cursor" => Target::Cursor,
            "copilot" | "github" | "github-copilot" => Target::Copilot,
            "agents" | "codex" | "amp" | "copilot-cli" => Target::Agents,
            other => Target::Custom(PathBuf::from(other)),
        }
    }
}
