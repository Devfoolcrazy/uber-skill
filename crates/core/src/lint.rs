//! Static checks on a library item.

use std::path::Path;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::frontmatter::SkillDoc;
use crate::fsutil;
use crate::library::{main_file, validate_id};
use crate::model::{ItemKind, Skill};
use crate::registry::{Facet, Registry};

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
    /// Family of the check, shown as a label.
    pub rule: &'static str,
    /// Stable identifier of the finding; front ends word it themselves from `args`.
    pub code: &'static str,
    pub args: Vec<String>,
    /// English, for the CLI and as a fallback.
    pub message: String,
    /// A replacement that settles the finding, when one is obvious.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<LinkFix>,
}

/// Rewrite the link target `from` as `to` in `file` (relative to the skill).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkFix {
    pub file: String,
    pub from: String,
    pub to: String,
}

/// Every finding the lint can report. Front ends word them by code.
pub const CODES: &[&str] = &[
    "file-missing",
    "frontmatter-invalid",
    "id-invalid",
    "name-missing",
    "name-mismatch",
    "description-missing",
    "description-too-long",
    "description-short",
    "tags-missing",
    "category-missing",
    "host-unknown",
    "body-empty",
    "body-long",
    "path-missing",
    "link-missing",
    "category-unknown",
    "tag-unknown",
];

impl Issue {
    fn new(
        severity: Severity,
        rule: &'static str,
        code: &'static str,
        args: Vec<String>,
        message: impl Into<String>,
    ) -> Issue {
        debug_assert!(CODES.contains(&code), "unlisted lint code {code}");
        Issue {
            severity,
            rule,
            code,
            args,
            message: message.into(),
            fix: None,
        }
    }
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
        issues.push(Issue::new(Severity::Error, "skill-file", "file-missing", vec![], what));
        return Ok(issues);
    }
    let text = fsutil::read_to_string(&md)?;
    let doc = match SkillDoc::parse(&text, &md) {
        Ok(d) => d,
        Err(e) => {
            issues.push(Issue::new(
                Severity::Error,
                "frontmatter",
                "frontmatter-invalid",
                vec![e.to_string()],
                e.to_string(),
            ));
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
        issues.push(Issue::new(
            Severity::Error,
            "dir-name",
            "id-invalid",
            vec![id.to_string()],
            format!("{holder} name `{id}`: {reason}"),
        ));
    }
    if meta.name.is_empty() {
        issues.push(Issue::new(
            Severity::Error,
            "name",
            "name-missing",
            vec![],
            "`name` is missing",
        ));
    } else if meta.name != id {
        issues.push(Issue::new(
            Severity::Error,
            "name",
            "name-mismatch",
            vec![meta.name.clone(), id.to_string()],
            format!("`name: {}` does not match {holder} `{id}`", meta.name),
        ));
    }
    if meta.description.trim().is_empty() {
        issues.push(Issue::new(
            Severity::Error,
            "description",
            "description-missing",
            vec![],
            "`description` is missing or empty",
        ));
    } else {
        let n = meta.description.chars().count();
        if n > MAX_DESCRIPTION {
            issues.push(Issue::new(
                Severity::Error,
                "description",
                "description-too-long",
                vec![n.to_string(), MAX_DESCRIPTION.to_string()],
                format!("description is {n} chars (max {MAX_DESCRIPTION})"),
            ));
        } else if n < 40 {
            issues.push(Issue::new(
                Severity::Warning,
                "description",
                "description-short",
                vec![],
                "description is very short; say what it does and when to use it",
            ));
        }
    }
    if meta.tags.is_empty() {
        issues.push(Issue::new(Severity::Info, "tags", "tags-missing", vec![], "no tags"));
    }
    if meta.category.is_none() {
        issues.push(Issue::new(
            Severity::Info,
            "category",
            "category-missing",
            vec![],
            "no category",
        ));
    }
    for h in &meta.hosts {
        if !crate::targets::Target::is_known_host(h) {
            issues.push(Issue::new(
                Severity::Warning,
                "hosts",
                "host-unknown",
                vec![h.clone()],
                format!("unknown host `{h}` (known: claude-code, codex, agents, amp, cursor, copilot)"),
            ));
        }
    }
    if doc.body.trim().is_empty() {
        issues.push(Issue::new(
            Severity::Error,
            "body",
            "body-empty",
            vec![],
            "the file has no body (system prompt / instructions)",
        ));
    } else {
        let lines = doc.body.lines().count();
        if kind == ItemKind::Skill && lines > RECOMMENDED_BODY_MAX_LINES {
            issues.push(Issue::new(
                Severity::Warning,
                "body",
                "body-long",
                vec![lines.to_string()],
                format!("body is {lines} lines; consider moving material into references/"),
            ));
        }
    }

    if kind == ItemKind::Skill {
        // Relative links of every markdown file, and code-spanned paths of SKILL.md,
        // should exist in the skill folder.
        let files = fsutil::list_files(path)?;
        check_links(
            path,
            &files,
            Path::new(crate::library::SKILL_FILE),
            &doc.body,
            &mut issues,
        );
        for file in files
            .iter()
            .filter(|f| f.extension().is_some_and(|e| e == "md") && path.join(f) != md)
        {
            if let Ok(text) = fsutil::read_to_string(&path.join(file)) {
                check_links(path, &files, file, &text, &mut issues);
            }
        }
        let path_re = Regex::new(r"`((?:scripts|references|assets|templates|tools)/[^`\s]+)`").unwrap();
        for cap in path_re.captures_iter(&doc.body) {
            let target = &cap[1];
            if !path.join(target).exists() {
                issues.push(Issue::new(
                    Severity::Warning,
                    "link",
                    "path-missing",
                    vec![target.to_string()],
                    format!("referenced file not found: {target}"),
                ));
            }
        }
    }

    issues.sort_by_key(|i| std::cmp::Reverse(i.severity));
    Ok(issues)
}

/// Report relative links of `text` (the markdown file `file`, relative to the
/// skill) that lead nowhere, suggesting a file of the same name when there is one.
fn check_links(skill: &Path, files: &[std::path::PathBuf], file: &Path, text: &str, issues: &mut Vec<Issue>) {
    let link_re = Regex::new(r"\]\(([^)\s#]+)(?:#[^)\s]*)?\)").unwrap();
    let base = file.parent().unwrap_or(Path::new(""));
    for cap in link_re.captures_iter(text) {
        let target = &cap[1];
        if target.contains(':') || target.starts_with('/') || target.contains('<') || target.contains('{') {
            continue;
        }
        if skill.join(base).join(target).exists() {
            continue;
        }
        let name = Path::new(target).file_name();
        // Written from the linking file: climb out of its folder, then down to the match.
        let suggestion = files
            .iter()
            .find(|f| f.file_name() == name)
            .map(|f| format!("{}{}", "../".repeat(base.components().count()), f.display()));
        let is_main = file == Path::new(crate::library::SKILL_FILE);
        let origin = if is_main {
            String::new()
        } else {
            format!(" (in {})", file.display())
        };
        let hint = suggestion
            .as_ref()
            .map(|s| format!("; did you mean {s}?"))
            .unwrap_or_default();
        let mut issue = Issue::new(
            Severity::Warning,
            "link",
            "link-missing",
            vec![
                target.to_string(),
                file.display().to_string(),
                suggestion.clone().unwrap_or_default(),
            ],
            format!("linked file not found: {target}{origin}{hint}"),
        );
        issue.fix = suggestion.map(|to| LinkFix {
            file: file.display().to_string(),
            from: target.to_string(),
            to,
        });
        issues.push(issue);
    }
}

/// Apply a link fix proposed by the lint: every link of `fix.file` pointing to
/// `fix.from` now points to `fix.to`. Only link targets are touched.
pub fn apply_link_fix(skill: &Path, fix: &LinkFix) -> Result<()> {
    let path = crate::library::safe_join(skill, &fix.file)?;
    // The new target must exist inside the skill: a fix never trades a broken link for another.
    let root = skill.canonicalize().map_err(|e| crate::Error::io(skill, e))?;
    let base = path.parent().unwrap_or(skill);
    let inside = base
        .join(&fix.to)
        .canonicalize()
        .is_ok_and(|target| target.starts_with(&root));
    if !inside {
        return Err(crate::Error::GitStale(crate::error::Stale::Item(fix.file.clone())));
    }
    let text = fsutil::read_to_string(&path)?;
    let link = Regex::new(&format!(r"\]\({}(#[^)\s]*)?\)", regex::escape(&fix.from))).unwrap();
    let fixed = link.replace_all(&text, |cap: &regex::Captures| {
        format!("]({}{})", fix.to, cap.get(1).map_or("", |m| m.as_str()))
    });
    fsutil::write_string(&path, &fixed)
}

/// Values the library's registry does not list. Warnings only: the registry
/// never rejects an item.
pub fn lint_registry(item: &Skill, registry: &Registry) -> Vec<Issue> {
    let unknown = |facet, rule, code, what: &str, value: &String| {
        (!registry.allows(facet, value)).then(|| {
            Issue::new(
                Severity::Warning,
                rule,
                code,
                vec![value.clone()],
                format!("{what} `{value}` is not in the library registry"),
            )
        })
    };
    let category = item
        .category
        .iter()
        .filter_map(|c| unknown(Facet::Category, "category", "category-unknown", "category", c));
    let tags = item
        .tags
        .iter()
        .filter_map(|t| unknown(Facet::Tag, "tags", "tag-unknown", "tag", t));
    category.chain(tags).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The desktop app owns the wording of findings, like it does for errors.
    #[test]
    fn desktop_app_words_every_finding() {
        let table = concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/desktop/src/lib/lint.ts");
        let table = std::fs::read_to_string(table).unwrap();
        for code in CODES {
            assert!(table.contains(&format!("\"{code}\":")), "no wording for {code}");
        }
    }

    #[test]
    fn checks_links_of_every_markdown_file_relative_to_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("forecasting");
        std::fs::create_dir_all(dir.join("chapters")).unwrap();
        std::fs::write(
            dir.join("SKILL.md"),
            "---\nname: forecasting\ndescription: Forecast time series with sound statistical methods.\n---\nSee [the cheatsheet](cheatsheet.md#rules), [the site](https://example.com) and [chapter one](ch01.md).\n",
        )
        .unwrap();
        std::fs::write(dir.join("cheatsheet.md"), "Go to [chapter one](chapters/ch01.md).\n").unwrap();
        std::fs::write(
            dir.join("chapters/ch01.md"),
            "Back to [the cheatsheet](../cheatsheet.md) or [the glossary](glossary.md).\n",
        )
        .unwrap();
        std::fs::write(dir.join("glossary.md"), "Terms.\n").unwrap();

        let links: Vec<String> = lint_dir(&dir)
            .unwrap()
            .into_iter()
            .filter(|i| i.rule == "link")
            .map(|i| i.message)
            .collect();
        assert_eq!(
            links,
            [
                "linked file not found: ch01.md; did you mean chapters/ch01.md?",
                "linked file not found: glossary.md (in chapters/ch01.md); did you mean ../glossary.md?",
            ]
        );

        // Applying the proposed fixes settles both findings and touches nothing else.
        let fixes: Vec<LinkFix> = lint_dir(&dir).unwrap().into_iter().filter_map(|i| i.fix).collect();
        assert_eq!(fixes.len(), 2);
        for fix in &fixes {
            apply_link_fix(&dir, fix).unwrap();
        }
        assert!(lint_dir(&dir).unwrap().iter().all(|i| i.rule != "link"));
        let chapter = std::fs::read_to_string(dir.join("chapters/ch01.md")).unwrap();
        assert_eq!(
            chapter,
            "Back to [the cheatsheet](../cheatsheet.md) or [the glossary](../glossary.md).\n"
        );
        let escaping = LinkFix {
            file: "SKILL.md".into(),
            from: "cheatsheet.md".into(),
            to: "../../etc/hosts".into(),
        };
        assert!(apply_link_fix(&dir, &escaping).is_err());
    }

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
