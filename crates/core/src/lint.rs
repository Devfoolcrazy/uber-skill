//! Static checks on a skill directory.

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::frontmatter::SkillDoc;
use crate::fsutil;
use crate::library::{validate_id, SKILL_FILE};

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

pub fn lint_dir(dir: &Path) -> Result<Vec<Issue>> {
    let mut issues = Vec::new();
    let skill_md = dir.join(SKILL_FILE);
    if !skill_md.is_file() {
        issues.push(Issue { severity: Severity::Error, rule: "skill-file", message: "SKILL.md is missing".into() });
        return Ok(issues);
    }
    let text = fsutil::read_to_string(&skill_md)?;
    let doc = match SkillDoc::parse(&text, &skill_md) {
        Ok(d) => d,
        Err(e) => {
            issues.push(Issue { severity: Severity::Error, rule: "frontmatter", message: e.to_string() });
            return Ok(issues);
        }
    };
    let meta = doc.meta();
    let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if let Err(reason) = validate_id(dir_name) {
        issues.push(Issue { severity: Severity::Error, rule: "dir-name", message: format!("directory name `{dir_name}`: {reason}") });
    }
    if meta.name.is_empty() {
        issues.push(Issue { severity: Severity::Error, rule: "name", message: "`name` is missing".into() });
    } else if meta.name != dir_name {
        issues.push(Issue { severity: Severity::Error, rule: "name", message: format!("`name: {}` does not match directory `{dir_name}`", meta.name) });
    }
    if meta.description.trim().is_empty() {
        issues.push(Issue { severity: Severity::Error, rule: "description", message: "`description` is missing or empty".into() });
    } else {
        let n = meta.description.chars().count();
        if n > MAX_DESCRIPTION {
            issues.push(Issue { severity: Severity::Error, rule: "description", message: format!("description is {n} chars (max {MAX_DESCRIPTION})") });
        } else if n < 40 {
            issues.push(Issue { severity: Severity::Warning, rule: "description", message: "description is very short; say what the skill does and when to use it".into() });
        }
    }
    if meta.tags.is_empty() {
        issues.push(Issue { severity: Severity::Info, rule: "tags", message: "no tags".into() });
    }
    if meta.category.is_none() {
        issues.push(Issue { severity: Severity::Info, rule: "category", message: "no category".into() });
    }
    for h in &meta.hosts {
        if !crate::targets::Target::is_known_host(h) {
            issues.push(Issue { severity: Severity::Warning, rule: "hosts", message: format!("unknown host `{h}` (known: claude-code, codex, agents, amp, cursor, copilot)") });
        }
    }
    if doc.body.trim().is_empty() {
        issues.push(Issue { severity: Severity::Error, rule: "body", message: "SKILL.md has no body".into() });
    } else {
        let lines = doc.body.lines().count();
        if lines > RECOMMENDED_BODY_MAX_LINES {
            issues.push(Issue { severity: Severity::Warning, rule: "body", message: format!("body is {lines} lines; consider moving material into references/") });
        }
    }

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
        let candidate = dir.join(target);
        if !candidate.exists() {
            issues.push(Issue { severity: Severity::Warning, rule: "link", message: format!("linked file not found: {target}") });
        }
    }
    let path_re = Regex::new(r"`((?:scripts|references|assets|templates|tools)/[^`\s]+)`").unwrap();
    for cap in path_re.captures_iter(&doc.body) {
        let target = &cap[1];
        if !dir.join(target).exists() {
            issues.push(Issue { severity: Severity::Warning, rule: "link", message: format!("referenced file not found: {target}") });
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
        std::fs::write(dir.join("SKILL.md"), "---\nname: other\ndescription: short\n---\nSee [x](references/x.md) and `scripts/a.py`.\n").unwrap();
        let issues = lint_dir(&dir).unwrap();
        let rules: Vec<_> = issues.iter().map(|i| i.rule).collect();
        assert!(rules.contains(&"name"));
        assert!(rules.iter().filter(|r| **r == "link").count() == 2);
        assert_eq!(issues[0].severity, Severity::Error);
    }
}
