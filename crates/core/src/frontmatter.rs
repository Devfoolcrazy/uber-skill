//! Parsing and serialization of the YAML frontmatter block of a `SKILL.md`.
//!
//! Tags and category live under `metadata` (the Agent Skills spec allows an arbitrary
//! string map there), so they stay portable across hosts. For reading we also accept
//! top-level `tags` / `category` keys.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_yaml::{Mapping, Value};

use crate::error::{Error, Result};

/// A parsed `SKILL.md`: the YAML mapping plus the markdown body.
#[derive(Debug, Clone, PartialEq)]
pub struct SkillDoc {
    pub front: Mapping,
    pub body: String,
}

/// The subset of frontmatter fields we interpret.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Meta {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Host tools this skill depends on (empty = works everywhere).
    #[serde(default)]
    pub hosts: Vec<String>,
}

/// Bodies of new items: purpose, instructions, usage example. Built in for now;
/// `{name}` is the only placeholder.
const SKILL_TEMPLATE: &str = include_str!("templates/skill.md");
const AGENT_TEMPLATE: &str = include_str!("templates/agent.md");

fn key(s: &str) -> Value {
    Value::String(s.to_string())
}

fn value_as_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Accepts `"a, b"` or `["a", "b"]`.
fn value_as_tags(v: &Value) -> Vec<String> {
    let raw: Vec<String> = match v {
        Value::Sequence(seq) => seq.iter().filter_map(value_as_string).collect(),
        Value::String(s) => s.split(',').map(|t| t.to_string()).collect(),
        _ => Vec::new(),
    };
    normalize_tags(raw)
}

pub fn normalize_tag(tag: &str) -> String {
    tag.trim().to_lowercase()
}

pub fn normalize_tags<I: IntoIterator<Item = String>>(tags: I) -> Vec<String> {
    let mut out: Vec<String> = tags
        .into_iter()
        .map(|t| normalize_tag(&t))
        .filter(|t| !t.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// A YAML scalar for `value`, quoted only when YAML requires it.
fn scalar(value: &str) -> Option<String> {
    let text = serde_yaml::to_string(&Value::String(value.to_string())).ok()?;
    let text = text.trim_end_matches('\n');
    (!text.contains('\n')).then(|| text.to_string())
}

/// Replace, in place, the single-line `tags`, `category` and `hosts` entries of
/// the `metadata` block. Returns None whenever the layout is anything else (a
/// key to add, a list or block value, legacy top-level keys): the caller then
/// re-serializes and verifies the result either way.
fn patch_metadata_lines(text: &str, meta: &Meta) -> Option<String> {
    let wanted: [(&str, Option<String>); 3] = [
        ("tags", (!meta.tags.is_empty()).then(|| meta.tags.join(", "))),
        ("category", meta.category.clone()),
        ("hosts", (!meta.hosts.is_empty()).then(|| meta.hosts.join(", "))),
    ];
    let mut lines: Vec<String> = Vec::new();
    let mut seen = [false; 3];
    let mut in_front = false;
    let mut in_metadata = false;
    let mut done = false;
    for (n, line) in text.split_inclusive('\n').enumerate() {
        let bare = line.trim_end_matches(['\r', '\n']);
        if done {
            lines.push(line.to_string());
            continue;
        }
        if bare == "---" {
            done = in_front;
            in_front = n == 0;
            lines.push(line.to_string());
            continue;
        }
        let indented = bare.starts_with([' ', '\t']);
        if !indented && !bare.is_empty() && !bare.starts_with('#') {
            in_metadata = bare.trim_end() == "metadata:";
        }
        let entry = in_metadata.then(|| bare.trim_start()).and_then(|entry| {
            wanted
                .iter()
                .position(|(k, _)| entry.strip_prefix(k).is_some_and(|rest| rest.starts_with(':')))
        });
        let Some(i) = entry.filter(|_| indented) else {
            lines.push(line.to_string());
            continue;
        };
        let (key, value) = &wanted[i];
        let current = bare.trim_start()[key.len() + 1..].trim();
        // Only plain one-line values; a comment, list or block scalar needs the full path.
        if seen[i]
            || current.is_empty()
            || current.starts_with(['[', '|', '>', '&', '*', '#'])
            || current.contains(" #")
        {
            return None;
        }
        seen[i] = true;
        if let Some(value) = value {
            let indent = &bare[..bare.len() - bare.trim_start().len()];
            let ending = &line[bare.len()..];
            lines.push(format!("{indent}{key}: {}{ending}", scalar(value)?));
        }
    }
    // A value to write without an existing line to carry it: not patchable.
    if wanted
        .iter()
        .zip(seen)
        .any(|((_, value), seen)| value.is_some() && !seen)
    {
        return None;
    }
    Some(lines.concat())
}

impl SkillDoc {
    /// Parse the text of a `SKILL.md`. `path` is only used for error messages.
    pub fn parse(text: &str, path: &Path) -> Result<SkillDoc> {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let Some(rest) = text.strip_prefix("---") else {
            return Err(Error::Frontmatter {
                path: path.to_path_buf(),
                reason: "file does not start with a `---` frontmatter block".into(),
            });
        };
        // The opening delimiter must be alone on its line.
        let rest = match rest.strip_prefix("\r\n").or_else(|| rest.strip_prefix('\n')) {
            Some(r) => r,
            None => {
                return Err(Error::Frontmatter {
                    path: path.to_path_buf(),
                    reason: "opening `---` must be on its own line".into(),
                })
            }
        };
        // Find the closing delimiter: a line that is exactly `---`.
        let mut offset = 0;
        let mut yaml_end = None;
        let mut body_start = None;
        for line in rest.split_inclusive('\n') {
            let trimmed = line.trim_end_matches(['\r', '\n']);
            if trimmed == "---" {
                yaml_end = Some(offset);
                body_start = Some(offset + line.len());
                break;
            }
            offset += line.len();
        }
        let (Some(yaml_end), Some(body_start)) = (yaml_end, body_start) else {
            return Err(Error::Frontmatter {
                path: path.to_path_buf(),
                reason: "closing `---` not found".into(),
            });
        };
        let yaml = &rest[..yaml_end];
        let body = rest[body_start..].to_string();
        let front: Value = if yaml.trim().is_empty() {
            Value::Mapping(Mapping::new())
        } else {
            serde_yaml::from_str(yaml).map_err(|e| Error::Frontmatter {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?
        };
        let front = match front {
            Value::Mapping(m) => m,
            Value::Null => Mapping::new(),
            _ => {
                return Err(Error::Frontmatter {
                    path: path.to_path_buf(),
                    reason: "frontmatter is not a mapping".into(),
                })
            }
        };
        Ok(SkillDoc { front, body })
    }

    /// Serialize back to `SKILL.md` text.
    pub fn to_text(&self) -> Result<String> {
        let yaml = serde_yaml::to_string(&Value::Mapping(self.front.clone()))?;
        let yaml = yaml.trim_end_matches('\n');
        let mut out = String::with_capacity(yaml.len() + self.body.len() + 16);
        out.push_str("---\n");
        out.push_str(yaml);
        out.push_str("\n---\n");
        out.push_str(&self.body);
        Ok(out)
    }

    pub fn get_str(&self, k: &str) -> Option<String> {
        self.front.get(key(k)).and_then(value_as_string)
    }

    fn metadata(&self) -> Option<&Mapping> {
        match self.front.get(key("metadata")) {
            Some(Value::Mapping(m)) => Some(m),
            _ => None,
        }
    }

    pub fn meta(&self) -> Meta {
        let md = self.metadata();
        let category = md
            .and_then(|m| m.get(key("category")))
            .or_else(|| self.front.get(key("category")))
            .and_then(value_as_string)
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty());
        let tags = md
            .and_then(|m| m.get(key("tags")))
            .or_else(|| self.front.get(key("tags")))
            .map(value_as_tags)
            .unwrap_or_default();
        let hosts = md
            .and_then(|m| m.get(key("hosts")))
            .map(value_as_tags)
            .unwrap_or_default();
        Meta {
            name: self.get_str("name").unwrap_or_default(),
            description: self.get_str("description").unwrap_or_default(),
            category,
            tags,
            hosts,
        }
    }

    /// Write tags, category and hosts under `metadata`, removing any legacy top-level keys.
    pub fn set_meta(&mut self, tags: &[String], category: Option<&str>, hosts: &[String]) {
        self.front.remove(key("tags"));
        self.front.remove(key("category"));
        let mut md = match self.front.remove(key("metadata")) {
            Some(Value::Mapping(m)) => m,
            _ => Mapping::new(),
        };
        let tags = normalize_tags(tags.iter().cloned());
        if tags.is_empty() {
            md.remove(key("tags"));
        } else {
            md.insert(key("tags"), Value::String(tags.join(", ")));
        }
        let hosts = normalize_tags(hosts.iter().cloned());
        if hosts.is_empty() {
            md.remove(key("hosts"));
        } else {
            md.insert(key("hosts"), Value::String(hosts.join(", ")));
        }
        match category.map(str::trim).filter(|c| !c.is_empty()) {
            Some(c) => {
                md.insert(key("category"), Value::String(c.to_string()));
            }
            None => {
                md.remove(key("category"));
            }
        }
        if !md.is_empty() {
            self.front.insert(key("metadata"), Value::Mapping(md));
        }
    }

    /// Text of the document with new tags, category and hosts. Rewrites only the
    /// matching `metadata` lines when that is enough, so comments, quoting and
    /// folded scalars elsewhere in the frontmatter survive and Git diffs stay
    /// small; otherwise falls back to a full re-serialization.
    pub fn text_with_meta(
        &self,
        text: &str,
        tags: &[String],
        category: Option<&str>,
        hosts: &[String],
    ) -> Result<String> {
        let mut next = self.clone();
        next.set_meta(tags, category, hosts);
        let expected = next.meta();
        if let Some(patched) = patch_metadata_lines(text, &expected) {
            let reparsed = SkillDoc::parse(&patched, Path::new("")).ok();
            if reparsed.is_some_and(|d| d.front == next.front && d.body == next.body) {
                return Ok(patched);
            }
        }
        next.to_text()
    }

    /// Text of the document with a new description, touching only its line when
    /// it is a plain one-line value; otherwise the frontmatter is re-serialized.
    pub fn text_with_description(&self, text: &str, description: &str) -> Result<String> {
        let mut next = self.clone();
        next.set_str("description", description);
        let patched = scalar(description).and_then(|value| {
            let mut found = false;
            let mut in_front = false;
            let mut lines = Vec::new();
            for (n, line) in text.split_inclusive('\n').enumerate() {
                let bare = line.trim_end_matches(['\r', '\n']);
                if bare == "---" {
                    in_front = n == 0;
                } else if in_front && !found {
                    if let Some(current) = bare.strip_prefix("description:") {
                        let current = current.trim();
                        if current.is_empty()
                            || current.starts_with(['|', '>', '&', '*', '#'])
                            || current.contains(" #")
                        {
                            return None;
                        }
                        found = true;
                        lines.push(format!("description: {value}{}", &line[bare.len()..]));
                        continue;
                    }
                }
                lines.push(line.to_string());
            }
            found.then(|| lines.concat())
        });
        if let Some(patched) = patched {
            let reparsed = SkillDoc::parse(&patched, Path::new("")).ok();
            if reparsed.is_some_and(|d| d.front == next.front && d.body == next.body) {
                return Ok(patched);
            }
        }
        next.to_text()
    }

    pub fn set_str(&mut self, k: &str, v: &str) {
        self.front.insert(key(k), Value::String(v.to_string()));
    }

    /// Build a fresh document for a new agent (Claude Code subagent format).
    pub fn new_agent(name: &str, description: &str) -> SkillDoc {
        let mut front = Mapping::new();
        front.insert(key("name"), Value::String(name.to_string()));
        front.insert(key("description"), Value::String(description.to_string()));
        SkillDoc {
            front,
            body: AGENT_TEMPLATE.replace("{name}", name),
        }
    }

    /// Build a fresh document for a new skill.
    pub fn new_skill(name: &str, description: &str) -> SkillDoc {
        let mut front = Mapping::new();
        front.insert(key("name"), Value::String(name.to_string()));
        front.insert(key("description"), Value::String(description.to_string()));
        SkillDoc {
            front,
            body: SKILL_TEMPLATE.replace("{name}", name),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p() -> std::path::PathBuf {
        "SKILL.md".into()
    }

    #[test]
    fn parses_metadata_tags_string_and_list() {
        let doc = SkillDoc::parse(
            "---\nname: x\ndescription: d\nmetadata:\n  tags: \"Rust, cli\"\n  category: Dev\n---\nbody\n",
            &p(),
        )
        .unwrap();
        let m = doc.meta();
        assert_eq!(m.tags, vec!["cli", "rust"]);
        assert_eq!(m.category.as_deref(), Some("Dev"));
        assert_eq!(doc.body, "body\n");

        let doc = SkillDoc::parse("---\nname: x\ndescription: d\ntags: [a, B]\n---\n", &p()).unwrap();
        assert_eq!(doc.meta().tags, vec!["a", "b"]);
    }

    #[test]
    fn roundtrip_preserves_unknown_keys() {
        let src = "---\nname: x\ndescription: d\nallowed-tools: Read, Grep\n---\n# hi\n";
        let mut doc = SkillDoc::parse(src, &p()).unwrap();
        doc.set_meta(&["b".into(), "a".into()], Some("cat"), &["Claude-Code".into()]);
        let text = doc.to_text().unwrap();
        let again = SkillDoc::parse(&text, &p()).unwrap();
        assert_eq!(again.get_str("allowed-tools").as_deref(), Some("Read, Grep"));
        assert_eq!(again.meta().tags, vec!["a", "b"]);
        assert_eq!(again.meta().category.as_deref(), Some("cat"));
        assert_eq!(again.meta().hosts, vec!["claude-code"]);
        assert_eq!(again.body, "# hi\n");
    }

    #[test]
    fn meta_edit_rewrites_only_its_lines_when_possible() {
        let src = "---\nname: x\n# keep this comment\ndescription: >\n  folded\n  text\nmetadata:\n  author: me\n  tags: git, old\n  category: \"Review\"\n---\n# body\n";
        let doc = SkillDoc::parse(src, &p()).unwrap();
        let out = doc
            .text_with_meta(src, &["git".into(), "new".into()], Some("review: deep"), &[])
            .unwrap();
        assert_eq!(
            out,
            "---\nname: x\n# keep this comment\ndescription: >\n  folded\n  text\nmetadata:\n  author: me\n  tags: git, new\n  category: 'review: deep'\n---\n# body\n"
        );
        // Removing a value drops its line and nothing else.
        let doc = SkillDoc::parse(&out, &p()).unwrap();
        let cleared = doc.text_with_meta(&out, &["git".into()], None, &[]).unwrap();
        assert!(
            cleared.contains("# keep this comment")
                && cleared.contains("  tags: git\n")
                && !cleared.contains("category")
        );
    }

    #[test]
    fn meta_edit_falls_back_to_serialization_and_stays_correct() {
        for src in [
            "---\nname: x\ndescription: d\n---\nbody\n",
            "---\nname: x\ndescription: d\ntags: [a, b]\n---\nbody\n",
            "---\nname: x\ndescription: d\nmetadata:\n  tags:\n    - a\n    - b\n---\nbody\n",
            "---\nname: x\ndescription: d\nmetadata:\n  tags: a # note\n---\nbody\n",
            "---\nname: x\ndescription: d\nmetadata:\n  tags: a\n---\nbody\n",
        ] {
            let doc = SkillDoc::parse(src, &p()).unwrap();
            let out = doc
                .text_with_meta(src, &["b".into(), "c".into()], Some("cat"), &["codex".into()])
                .unwrap();
            let meta = SkillDoc::parse(&out, &p()).unwrap().meta();
            assert_eq!(
                (meta.tags, meta.category.as_deref(), meta.hosts),
                (vec!["b".into(), "c".into()], Some("cat"), vec!["codex".to_string()]),
                "{src}"
            );
            assert!(out.ends_with("---\nbody\n"));
        }
    }

    #[test]
    fn rejects_missing_frontmatter() {
        assert!(SkillDoc::parse("# no front\n", &p()).is_err());
        assert!(SkillDoc::parse("---\nname: x\n", &p()).is_err());
    }
}
