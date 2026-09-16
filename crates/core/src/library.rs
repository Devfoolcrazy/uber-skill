//! The skill library: a directory tree where every folder containing a `SKILL.md`
//! is a skill. The filesystem is the source of truth; scanning builds the index.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::frontmatter::{normalize_tags, SkillDoc};
use crate::fsutil::{self, hash_dir, is_ignored, list_files};
use crate::model::{ScanResult, ScanWarning, Skill};

pub const SKILL_FILE: &str = "SKILL.md";

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
}

/// Valid skill ids: lowercase letters, digits and hyphens, 1..=64 chars,
/// no leading/trailing/double hyphen.
pub fn validate_id(id: &str) -> std::result::Result<(), String> {
    if id.is_empty() || id.len() > 64 {
        return Err("must be 1 to 64 characters".into());
    }
    if !id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-') {
        return Err("only lowercase letters, digits and hyphens are allowed".into());
    }
    if id.starts_with('-') || id.ends_with('-') || id.contains("--") {
        return Err("hyphens cannot be leading, trailing or doubled".into());
    }
    Ok(())
}

/// Read a single skill directory into a [`Skill`].
pub fn read_skill_dir(dir: &Path, library_root: Option<&Path>) -> Result<Skill> {
    let skill_md = dir.join(SKILL_FILE);
    let text = fsutil::read_to_string(&skill_md)?;
    let doc = SkillDoc::parse(&text, &skill_md)?;
    let meta = doc.meta();
    let id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| Error::NotADirectory(dir.to_path_buf()))?
        .to_string();
    let files = list_files(dir)?
        .into_iter()
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .collect();
    let extra = doc
        .front
        .iter()
        .filter_map(|(k, v)| {
            let k = k.as_str()?;
            if matches!(k, "name" | "description" | "metadata" | "tags" | "category" | "hosts") {
                return None;
            }
            let v = match v {
                serde_yaml::Value::String(s) => s.clone(),
                serde_yaml::Value::Bool(b) => b.to_string(),
                serde_yaml::Value::Number(n) => n.to_string(),
                other => serde_yaml::to_string(other).unwrap_or_default().trim().to_string(),
            };
            Some((k.to_string(), v))
        })
        .collect::<BTreeMap<_, _>>();
    let modified_at = std::fs::metadata(&skill_md)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| DateTime::<Utc>::from(t).to_rfc3339());
    let rel_path = match library_root {
        Some(root) => dir.strip_prefix(root).unwrap_or(dir).to_path_buf(),
        None => PathBuf::from(&id),
    };
    Ok(Skill {
        id,
        name: meta.name,
        description: meta.description,
        category: meta.category,
        tags: meta.tags,
        hosts: meta.hosts,
        path: dir.to_path_buf(),
        rel_path,
        hash: hash_dir(dir)?,
        files,
        extra,
        body_chars: doc.body.chars().count(),
        modified_at,
    })
}

impl Library {
    pub fn open(root: impl Into<PathBuf>) -> Result<Library> {
        let root: PathBuf = root.into();
        if !root.is_dir() {
            return Err(Error::NotADirectory(root));
        }
        let root = root.canonicalize().map_err(|e| Error::io(&root, e))?;
        Ok(Library { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn skill_dir(&self, id: &str) -> Result<PathBuf> {
        validate_id(id).map_err(|_| Error::InvalidId(id.to_string()))?;
        // A skill may be nested inside category folders: find it by scanning.
        for entry in WalkDir::new(&self.root)
            .follow_links(true)
            .min_depth(1)
            .max_depth(6)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_str().map(is_ignored).unwrap_or(false))
            .flatten()
        {
            if entry.file_type().is_dir()
                && entry.file_name() == id
                && entry.path().join(SKILL_FILE).is_file()
            {
                return Ok(entry.path().to_path_buf());
            }
        }
        Err(Error::SkillNotFound(id.to_string()))
    }

    /// Scan the whole library. Skill directories are not descended into.
    pub fn scan(&self) -> Result<ScanResult> {
        let mut result = ScanResult::default();
        let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();
        let mut it = WalkDir::new(&self.root)
            .follow_links(true)
            .min_depth(1)
            .max_depth(6)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(|e| !e.file_name().to_str().map(is_ignored).unwrap_or(false));
        while let Some(entry) = it.next() {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    result.warnings.push(ScanWarning {
                        path: e.path().map(Path::to_path_buf).unwrap_or_default(),
                        message: e.to_string(),
                    });
                    continue;
                }
            };
            if !entry.file_type().is_dir() || !entry.path().join(SKILL_FILE).is_file() {
                continue;
            }
            it.skip_current_dir();
            let dir = entry.path();
            match read_skill_dir(dir, Some(&self.root)) {
                Ok(skill) => {
                    if let Some(prev) = seen.get(&skill.id) {
                        result.warnings.push(ScanWarning {
                            path: dir.to_path_buf(),
                            message: format!(
                                "duplicate skill id `{}` (already at {})",
                                skill.id,
                                prev.display()
                            ),
                        });
                        continue;
                    }
                    seen.insert(skill.id.clone(), dir.to_path_buf());
                    result.skills.push(skill);
                }
                Err(e) => result.warnings.push(ScanWarning {
                    path: dir.to_path_buf(),
                    message: e.to_string(),
                }),
            }
        }
        Ok(result)
    }

    pub fn get(&self, id: &str) -> Result<Skill> {
        let dir = self.skill_dir(id)?;
        read_skill_dir(&dir, Some(&self.root))
    }

    /// Create a new skill folder with a minimal SKILL.md. `category` becomes a subfolder.
    pub fn create(&self, id: &str, description: &str, category: Option<&str>, tags: &[String], hosts: &[String]) -> Result<Skill> {
        validate_id(id).map_err(|_| Error::InvalidId(id.to_string()))?;
        if self.skill_dir(id).is_ok() {
            return Err(Error::SkillExists(id.to_string()));
        }
        let dir = self.root.join(id);
        let mut doc = SkillDoc::new_skill(id, description);
        doc.set_meta(tags, category, hosts);
        fsutil::write_string(&dir.join(SKILL_FILE), &doc.to_text()?)?;
        read_skill_dir(&dir, Some(&self.root))
    }

    pub fn read_skill_md(&self, id: &str) -> Result<String> {
        let dir = self.skill_dir(id)?;
        fsutil::read_to_string(&dir.join(SKILL_FILE))
    }

    /// Overwrite SKILL.md after validating that it still parses.
    pub fn write_skill_md(&self, id: &str, text: &str) -> Result<Skill> {
        let dir = self.skill_dir(id)?;
        let path = dir.join(SKILL_FILE);
        SkillDoc::parse(text, &path)?;
        fsutil::write_string(&path, text)?;
        read_skill_dir(&dir, Some(&self.root))
    }

    pub fn read_file(&self, id: &str, rel: &str) -> Result<String> {
        let dir = self.skill_dir(id)?;
        let path = safe_join(&dir, rel)?;
        fsutil::read_to_string(&path)
    }

    pub fn write_file(&self, id: &str, rel: &str, text: &str) -> Result<()> {
        let dir = self.skill_dir(id)?;
        let path = safe_join(&dir, rel)?;
        if rel == SKILL_FILE {
            SkillDoc::parse(text, &path)?;
        }
        fsutil::write_string(&path, text)
    }

    /// Update tags/category/hosts (and optionally description) in the frontmatter.
    /// `None` keeps the current value; `category: Some(None)` clears it.
    pub fn update_meta(
        &self,
        id: &str,
        tags: Option<&[String]>,
        category: Option<Option<&str>>,
        hosts: Option<&[String]>,
        description: Option<&str>,
    ) -> Result<Skill> {
        let dir = self.skill_dir(id)?;
        let path = dir.join(SKILL_FILE);
        let text = fsutil::read_to_string(&path)?;
        let mut doc = SkillDoc::parse(&text, &path)?;
        let current = doc.meta();
        let tags: Vec<String> = match tags {
            Some(t) => normalize_tags(t.iter().cloned()),
            None => current.tags.clone(),
        };
        let category: Option<String> = match category {
            Some(c) => c.map(|s| s.to_string()),
            None => current.category.clone(),
        };
        let hosts: Vec<String> = match hosts {
            Some(h) => normalize_tags(h.iter().cloned()),
            None => current.hosts.clone(),
        };
        doc.set_meta(&tags, category.as_deref(), &hosts);
        if let Some(d) = description {
            doc.set_str("description", d);
        }
        fsutil::write_string(&path, &doc.to_text()?)?;
        read_skill_dir(&dir, Some(&self.root))
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let dir = self.skill_dir(id)?;
        std::fs::remove_dir_all(&dir).map_err(|e| Error::io(&dir, e))
    }

    /// Import an external skill directory into the library (copy).
    pub fn import(&self, src: &Path, new_id: Option<&str>) -> Result<Skill> {
        let src_skill = read_skill_dir(src, None)?;
        let id = new_id.unwrap_or(&src_skill.id);
        validate_id(id).map_err(|_| Error::InvalidId(id.to_string()))?;
        if self.skill_dir(id).is_ok() {
            return Err(Error::SkillExists(id.to_string()));
        }
        let dst = self.root.join(id);
        fsutil::copy_dir(src, &dst)?;
        read_skill_dir(&dst, Some(&self.root))
    }

    /// All tags and categories present in the library, for filters.
    pub fn facets(skills: &[Skill]) -> (Vec<String>, Vec<String>) {
        let tags: BTreeSet<&String> = skills.iter().flat_map(|s| s.tags.iter()).collect();
        let cats: BTreeSet<&String> = skills.iter().filter_map(|s| s.category.as_ref()).collect();
        (
            tags.into_iter().cloned().collect(),
            cats.into_iter().cloned().collect(),
        )
    }
}

/// Join `rel` under `base` refusing `..` and absolute paths.
pub fn safe_join(base: &Path, rel: &str) -> Result<PathBuf> {
    let rel_path = Path::new(rel);
    if rel_path.is_absolute()
        || rel_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir | std::path::Component::Prefix(_)))
    {
        return Err(Error::InvalidId(format!("unsafe relative path: {rel}")));
    }
    Ok(base.join(rel_path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn scans_nested_and_flags_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        write(&r.join("alpha/SKILL.md"), "---\nname: alpha\ndescription: A\nmetadata:\n  tags: x, y\n---\nbody");
        write(&r.join("cat/beta/SKILL.md"), "---\nname: beta\ndescription: B\n---\n");
        write(&r.join("other/alpha/SKILL.md"), "---\nname: alpha\ndescription: dup\n---\n");
        write(&r.join("broken/SKILL.md"), "no frontmatter");
        write(&r.join(".git/config"), "x");
        let lib = Library::open(r).unwrap();
        let res = lib.scan().unwrap();
        let ids: Vec<_> = res.skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta"]);
        assert_eq!(res.warnings.len(), 2, "{:?}", res.warnings);
        assert_eq!(res.skills[0].tags, vec!["x", "y"]);
        assert_eq!(res.skills[1].rel_path, PathBuf::from("cat/beta"));
    }

    #[test]
    fn create_update_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(tmp.path()).unwrap();
        let s = lib.create("my-skill", "does things", Some("dev"), &["Rust".into()], &[]).unwrap();
        assert_eq!(s.tags, vec!["rust"]);
        assert_eq!(s.category.as_deref(), Some("dev"));
        let s = lib.update_meta("my-skill", Some(&["a".into(), "b".into()]), Some(None), Some(&["codex".into()]), None).unwrap();
        assert_eq!(s.tags, vec!["a", "b"]);
        assert_eq!(s.category, None);
        assert_eq!(s.hosts, vec!["codex"]);
        let s = lib.update_meta("my-skill", None, None, Some(&[]), None).unwrap();
        assert!(s.hosts.is_empty());
        assert!(lib.create("my-skill", "x", None, &[], &[]).is_err());
        assert!(lib.create("Bad Id", "x", None, &[], &[]).is_err());
        lib.delete("my-skill").unwrap();
        assert!(lib.get("my-skill").is_err());
    }

    #[test]
    fn safe_join_rejects_escape() {
        assert!(safe_join(Path::new("/x"), "../etc").is_err());
        assert!(safe_join(Path::new("/x"), "/etc").is_err());
        assert!(safe_join(Path::new("/x"), "a/b.md").is_ok());
    }
}
