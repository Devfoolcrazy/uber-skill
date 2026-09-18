//! Static checks on a library item.

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::frontmatter::SkillDoc;
use crate::fsutil;
use crate::library::{main_file, validate_id};
use crate::model::ItemKind;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Issue {
    pub severity: Severity,
    pub rule: &'static str,
    pub message: String,
}

pub const MAX_DESCRIPTION: usize = 1024;
pub const RECOMMENDED_BODY_MAX_LINES: usize = 500;

/// Lint a skill directory.
pub fn lint_dir(dir: &Path) -> Result<Vec<Issue>> {
    lint_item(dir, ItemKind::Skill)
}

/// Lint a skill directory or an agent file.
pub fn lint_item(path: &Path, kind: ItemKind) -> Result<Vec<Issue>> {
    let mut issues = Vec::new();
    let md = main_file(path, kind);
    if !md.is_file() {
        let what = match kind {
            ItemKind::Skill => "SKILL.md is missing",
            ItemKind::Agent => "agent file is missing",
        };
        issues.push(Issue {
            severity: Severity::Error,
            rule: "skill-file",
            message: what.into(),
        });
        return Ok(issues);
    }
    let text = fsutil::read_to_string(&md)?;
    let doc = match SkillDoc::parse(&text, &md) {
        Ok(d) => d,
        Err(e) => {
            issues.push(Issue {
                severity: Severity::Error,
                rule: "frontmatter",
                message: e.to_string(),
            });
            return Ok(issues);
        }
    };
    let meta = doc.meta();
    let id = match kind {
        ItemKind::Skill => path.file_name(),
        ItemKind::Agent => path.file_stem(),
    }
    .and_then(|n| n.to_str())
    .unwrap_or("");
    let holder = match kind {
        ItemKind::Skill => "directory",
        ItemKind::Agent => "file",
    };

    if let Err(reason) = validate_id(id) {
        issues.push(Issue {
            severity: Severity::Error,
            rule: "dir-name",
            message: format!("{holder} name `{id}`: {reason}"),
        });
    }
    if meta.name.is_empty() {
        issues.push(Issue {
            severity: Severity::Error,
            rule: "name",
            message: "`name` is missing".into(),
        });
    } else if meta.name != id {
        issues.push(Issue {
            severity: Severity::Error,
            rule: "name",
            message: format!("`name: {}` does not match {holder} `{id}`", meta.name),
        });
    }
    if meta.description.trim().is_empty() {
        issues.push(Issue {
            severity: Severity::Error,
            rule: "description",
            message: "`description` is missing or empty".into(),
        });
    } else {
        let n = meta.description.chars().count();
        if n > MAX_DESCRIPTION {
            issues.push(Issue {
                severity: Severity::Error,
                rule: "description",
                message: format!("description is {n} chars (max {MAX_DESCRIPTION})"),
            });
        } else if n < 40 {
            issues.push(Issue {
                severity: Severity::Warning,
                rule: "description",
                message: "description is very short; say what it does and when to use it".into(),
            });
        }
    }
    if meta.tags.is_empty() {
        issues.push(Issue {
            severity: Severity::Info,
            rule: "tags",
            message: "no tags".into(),
        });
    }
    if meta.category.is_none() {
        issues.push(Issue {
            severity: Severity::Info,
            rule: "category",
            message: "no category".into(),
        });
    }
    for h in &meta.hosts {
        if !crate::targets::Target::is_known_host(h) {
            issues.push(Issue {
                severity: Severity::Warning,
                rule: "hosts",
                message: format!("unknown host `{h}` (known: claude-code, codex, agents, amp, cursor, copilot)"),
            });
        }
    }
    if doc.body.trim().is_empty() {
        issues.push(Issue {
            severity: Severity::Error,
            rule: "body",
            message: "the file has no body (system prompt / instructions)".into(),
        });
    } else {
        let lines = doc.body.lines().count();
        if kind == ItemKind::Skill && lines > RECOMMENDED_BODY_MAX_LINES {
            issues.push(Issue {
                severity: Severity::Warning,
                rule: "body",
                message: format!("body is {lines} lines; consider moving material into references/"),
            });
        }
    }

    if kind == ItemKind::Skill {
        // Relative links and code-spanned paths that should exist in the skill folder.
        let link_re = Regex::new(r"\]\(([^)\s#]+)\)").unwrap();
        for cap in link_re.captures_iter(&doc.body) {
            let target = &cap[1];
            if target.contains("://")
                || target.starts_with('/')
                || target.starts_with("mailto:")
                || target.contains('<')
                || target.contains('{')
            {
                continue;
            }
            if !path.join(target).exists() {
                issues.push(Issue {
                    severity: Severity::Warning,
                    rule: "link",
                    message: format!("linked file not found: {target}"),
                });
            }
        }
        let path_re = Regex::new(r"`((?:scripts|references|assets|templates|tools)/[^`\s]+)`").unwrap();
        for cap in path_re.captures_iter(&doc.body) {
            let target = &cap[1];
            if !path.join(target).exists() {
                issues.push(Issue {
                    severity: Severity::Warning,
                    rule: "link",
                    message: format!("referenced file not found: {target}"),
                });
            }
        }
    }

    issues.sort_by_key(|i| std::cmp::Reverse(i.severity));
    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_common_problems() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("my-skill");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: other\ndescription: short\n---\nSee [x](references/x.md) and `scripts/a.py`.\n",
        )
        .unwrap();
        let issues = lint_dir(&dir).unwrap();
        let rules: Vec<_> = issues.iter().map(|i| i.rule).collect();
        assert!(rules.contains(&"name"));
        assert!(rules.iter().filter(|r| **r == "link").count() == 2);
        assert_eq!(issues[0].severity, Severity::Error);

        let agent = tmp.path().join("coder.md");
        std::fs::write(
            &agent,
            "---\nname: coder\ndescription: Implements code from a spec written by the orchestrator.\n---\n",
        )
        .unwrap();
        let issues = lint_item(&agent, ItemKind::Agent).unwrap();
        assert!(issues.iter().any(|i| i.rule == "body" && i.severity == Severity::Error));
        assert!(!issues.iter().any(|i| i.rule == "name"));
    }
}
