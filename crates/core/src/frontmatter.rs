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
            body: format!("\nTu es {name}.\n\n## Périmètre\n\n## Méthode\n"),
        }
    }

    /// Build a fresh document for a new skill.
    pub fn new_skill(name: &str, description: &str) -> SkillDoc {
        let mut front = Mapping::new();
        front.insert(key("name"), Value::String(name.to_string()));
        front.insert(key("description"), Value::String(description.to_string()));
        SkillDoc {
            front,
            body: format!("\n# {name}\n\n## Quand utiliser\n\n## Instructions\n"),
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
    fn rejects_missing_frontmatter() {
        assert!(SkillDoc::parse("# no front\n", &p()).is_err());
        assert!(SkillDoc::parse("---\nname: x\n", &p()).is_err());
    }
}
