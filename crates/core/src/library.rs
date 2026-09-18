//! A library of items. For skills: a directory tree where every folder containing a
//! `SKILL.md` is a skill. For agents: every `<name>.md` file with frontmatter is an agent.
//! The filesystem is the source of truth; scanning builds the index.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::frontmatter::{normalize_tags, SkillDoc};
use crate::fsutil::{self, hash_path, is_ignored, list_files};
use crate::model::{ItemKind, ScanResult, ScanWarning, Skill};

pub const SKILL_FILE: &str = "SKILL.md";

type Walker = walkdir::FilterEntry<walkdir::IntoIter, Box<dyn FnMut(&walkdir::DirEntry) -> bool>>;

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    kind: ItemKind,
}

/// Valid item ids: lowercase letters, digits and hyphens, 1..=64 chars,
/// no leading/trailing/double hyphen.
pub fn validate_id(id: &str) -> std::result::Result<(), String> {
    if id.is_empty() || id.len() > 64 {
        return Err("must be 1 to 64 characters".into());
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    {
        return Err("only lowercase letters, digits and hyphens are allowed".into());
    }
    if id.starts_with('-') || id.ends_with('-') || id.contains("--") {
        return Err("hyphens cannot be leading, trailing or doubled".into());
    }
    Ok(())
}

/// Folders starting with `_` (e.g. `_AGENTS`) are never scanned as skills.
fn skip_in_skill_scan(name: &str) -> bool {
    is_ignored(name) || name.starts_with('_')
}

/// The main markdown file of an item: `<dir>/SKILL.md` or the agent file itself.
pub fn main_file(path: &Path, kind: ItemKind) -> PathBuf {
    match kind {
        ItemKind::Skill => path.join(SKILL_FILE),
        ItemKind::Agent => path.to_path_buf(),
    }
}

fn id_of(path: &Path, kind: ItemKind) -> Result<String> {
    let name = match kind {
        ItemKind::Skill => path.file_name(),
        ItemKind::Agent => path.file_stem(),
    };
    name.and_then(|n| n.to_str())
        .map(str::to_string)
        .ok_or_else(|| Error::NotADirectory(path.to_path_buf()))
}

/// Read a single item (skill directory or agent file) into a [`Skill`].
pub fn read_item(path: &Path, library_root: Option<&Path>, kind: ItemKind) -> Result<Skill> {
    let md = main_file(path, kind);
    let text = fsutil::read_to_string(&md)?;
    let doc = SkillDoc::parse(&text, &md)?;
    let meta = doc.meta();
    let id = id_of(path, kind)?;
    let files: Vec<String> = match kind {
        ItemKind::Skill => list_files(path)?
            .into_iter()
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .collect(),
        ItemKind::Agent => vec![md
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default()],
    };
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
    let modified_at = std::fs::metadata(&md)
        .and_then(|m| m.modified())
        .ok()
        .map(|t| DateTime::<Utc>::from(t).to_rfc3339());
    let rel_path = match library_root {
        Some(root) => path.strip_prefix(root).unwrap_or(path).to_path_buf(),
        None => PathBuf::from(&id),
    };
    Ok(Skill {
        kind,
        id,
        name: meta.name,
        description: meta.description,
        category: meta.category,
        tags: meta.tags,
        hosts: meta.hosts,
        path: path.to_path_buf(),
        rel_path,
        hash: hash_path(path)?,
        files,
        extra,
        body_chars: doc.body.chars().count(),
        modified_at,
    })
}

/// Read a single skill directory into a [`Skill`].
pub fn read_skill_dir(dir: &Path, library_root: Option<&Path>) -> Result<Skill> {
    read_item(dir, library_root, ItemKind::Skill)
}

impl Library {
    /// Open a skills library.
    pub fn open(root: impl Into<PathBuf>) -> Result<Library> {
        Library::open_kind(root, ItemKind::Skill)
    }

    pub fn open_kind(root: impl Into<PathBuf>, kind: ItemKind) -> Result<Library> {
        let root: PathBuf = root.into();
        if !root.is_dir() {
            return Err(Error::NotADirectory(root));
        }
        let root = root.canonicalize().map_err(|e| Error::io(&root, e))?;
        Ok(Library { root, kind })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn kind(&self) -> ItemKind {
        self.kind
    }

    fn walker(&self) -> Walker {
        let kind = self.kind;
        let pred: Box<dyn FnMut(&walkdir::DirEntry) -> bool> = Box::new(move |e| {
            let name = e.file_name().to_str().unwrap_or("");
            match kind {
                ItemKind::Skill => !skip_in_skill_scan(name),
                ItemKind::Agent => !is_ignored(name),
            }
        });
        WalkDir::new(&self.root)
            .follow_links(true)
            .min_depth(1)
            .max_depth(6)
            .sort_by_file_name()
            .into_iter()
            .filter_entry(pred)
    }

    fn is_item(&self, entry: &walkdir::DirEntry) -> bool {
        match self.kind {
            ItemKind::Skill => entry.file_type().is_dir() && entry.path().join(SKILL_FILE).is_file(),
            ItemKind::Agent => {
                entry.file_type().is_file()
                    && entry.path().extension().map(|e| e == "md").unwrap_or(false)
                    && !entry
                        .file_name()
                        .to_str()
                        .unwrap_or("")
                        .starts_with(|c: char| c.is_ascii_uppercase())
            }
        }
    }

    /// Path of an item by id (skill dir or agent file). Items may be nested in subfolders.
    pub fn item_path(&self, id: &str) -> Result<PathBuf> {
        validate_id(id).map_err(|_| Error::InvalidId(id.to_string()))?;
        for entry in self.walker().flatten() {
            if !self.is_item(&entry) {
                continue;
            }
            let matches = match self.kind {
                ItemKind::Skill => entry.file_name() == id,
                ItemKind::Agent => entry.path().file_stem().map(|s| s == id).unwrap_or(false),
            };
            if matches {
                return Ok(entry.path().to_path_buf());
            }
        }
        Err(Error::SkillNotFound(id.to_string()))
    }

    /// Path of a skill directory by id.
    pub fn skill_dir(&self, id: &str) -> Result<PathBuf> {
        self.item_path(id)
    }

    /// Scan the whole library. Skill directories are not descended into.
    pub fn scan(&self) -> Result<ScanResult> {
        let mut result = ScanResult::default();
        let mut seen: BTreeMap<String, PathBuf> = BTreeMap::new();
        let mut it = self.walker();
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
            if !self.is_item(&entry) {
                continue;
            }
            if self.kind == ItemKind::Skill {
                it.skip_current_dir();
            }
            let path = entry.path();
            match read_item(path, Some(&self.root), self.kind) {
                Ok(item) => {
                    if let Some(prev) = seen.get(&item.id) {
                        result.warnings.push(ScanWarning {
                            path: path.to_path_buf(),
                            message: format!(
                                "duplicate {} id `{}` (already at {})",
                                self.kind.label(),
                                item.id,
                                prev.display()
                            ),
                        });
                        continue;
                    }
                    seen.insert(item.id.clone(), path.to_path_buf());
                    result.skills.push(item);
                }
                Err(e) => result.warnings.push(ScanWarning {
                    path: path.to_path_buf(),
                    message: e.to_string(),
                }),
            }
        }
        Ok(result)
    }

    pub fn get(&self, id: &str) -> Result<Skill> {
        let path = self.item_path(id)?;
        read_item(&path, Some(&self.root), self.kind)
    }

    fn new_item_path(&self, id: &str) -> PathBuf {
        match self.kind {
            ItemKind::Skill => self.root.join(id),
            ItemKind::Agent => self.root.join(format!("{id}.md")),
        }
    }

    /// Create a new item with a minimal main file at the library root.
    pub fn create(
        &self,
        id: &str,
        description: &str,
        category: Option<&str>,
        tags: &[String],
        hosts: &[String],
    ) -> Result<Skill> {
        validate_id(id).map_err(|_| Error::InvalidId(id.to_string()))?;
        if self.item_path(id).is_ok() {
            return Err(Error::SkillExists(id.to_string()));
        }
        let path = self.new_item_path(id);
        let mut doc = match self.kind {
            ItemKind::Skill => SkillDoc::new_skill(id, description),
            ItemKind::Agent => SkillDoc::new_agent(id, description),
        };
        doc.set_meta(tags, category, hosts);
        fsutil::write_string(&main_file(&path, self.kind), &doc.to_text()?)?;
        read_item(&path, Some(&self.root), self.kind)
    }

    /// Read the main markdown file (SKILL.md or the agent file).
    pub fn read_main(&self, id: &str) -> Result<String> {
        let path = self.item_path(id)?;
        fsutil::read_to_string(&main_file(&path, self.kind))
    }

    /// Overwrite the main markdown file after validating that it still parses.
    pub fn write_main(&self, id: &str, text: &str) -> Result<Skill> {
        let path = self.item_path(id)?;
        let md = main_file(&path, self.kind);
        SkillDoc::parse(text, &md)?;
        fsutil::write_string(&md, text)?;
        read_item(&path, Some(&self.root), self.kind)
    }

    /// Resolve `rel` inside an item: any file under a skill dir; only the agent file itself for agents.
    fn resolve_file(&self, id: &str, rel: &str) -> Result<PathBuf> {
        let path = self.item_path(id)?;
        match self.kind {
            ItemKind::Skill => safe_join(&path, rel),
            ItemKind::Agent => {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if rel == name || rel.is_empty() {
                    Ok(path)
                } else {
                    Err(Error::InvalidId(format!(
                        "agents have a single file ({name}), not {rel}"
                    )))
                }
            }
        }
    }

    pub fn read_file(&self, id: &str, rel: &str) -> Result<String> {
        fsutil::read_to_string(&self.resolve_file(id, rel)?)
    }

    pub fn write_file(&self, id: &str, rel: &str, text: &str) -> Result<()> {
        let path = self.resolve_file(id, rel)?;
        let is_main = path == main_file(&self.item_path(id)?, self.kind);
        if is_main {
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
        let path = self.item_path(id)?;
        let md = main_file(&path, self.kind);
        let text = fsutil::read_to_string(&md)?;
        let mut doc = SkillDoc::parse(&text, &md)?;
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
        fsutil::write_string(&md, &doc.to_text()?)?;
        read_item(&path, Some(&self.root), self.kind)
    }

    pub fn delete(&self, id: &str) -> Result<()> {
        let path = self.item_path(id)?;
        if path.is_dir() {
            std::fs::remove_dir_all(&path).map_err(|e| Error::io(&path, e))
        } else {
            std::fs::remove_file(&path).map_err(|e| Error::io(&path, e))
        }
    }

    /// Import an external item (skill directory or agent file) into the library (copy).
    pub fn import(&self, src: &Path, new_id: Option<&str>) -> Result<Skill> {
        let src_item = read_item(src, None, self.kind)?;
        let id = new_id.unwrap_or(&src_item.id);
        validate_id(id).map_err(|_| Error::InvalidId(id.to_string()))?;
        if self.item_path(id).is_ok() {
            return Err(Error::SkillExists(id.to_string()));
        }
        let dst = self.new_item_path(id);
        fsutil::copy_item(src, &dst)?;
        read_item(&dst, Some(&self.root), self.kind)
    }

    /// All tags and categories present in the library, for filters.
    pub fn facets(skills: &[Skill]) -> (Vec<String>, Vec<String>) {
        let tags: BTreeSet<&String> = skills.iter().flat_map(|s| s.tags.iter()).collect();
        let cats: BTreeSet<&String> = skills.iter().filter_map(|s| s.category.as_ref()).collect();
        (tags.into_iter().cloned().collect(), cats.into_iter().cloned().collect())
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
        write(
            &r.join("alpha/SKILL.md"),
            "---\nname: alpha\ndescription: A\nmetadata:\n  tags: x, y\n---\nbody",
        );
        write(&r.join("cat/beta/SKILL.md"), "---\nname: beta\ndescription: B\n---\n");
        write(
            &r.join("other/alpha/SKILL.md"),
            "---\nname: alpha\ndescription: dup\n---\n",
        );
        write(&r.join("broken/SKILL.md"), "no frontmatter");
        write(&r.join(".git/config"), "x");
        write(
            &r.join("_AGENTS/coder.md"),
            "---\nname: coder\ndescription: C\n---\nagent",
        );
        write(&r.join("_AGENTS/README.md"), "not an agent");
        let lib = Library::open(r).unwrap();
        let res = lib.scan().unwrap();
        let ids: Vec<_> = res.skills.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, vec!["alpha", "beta"]);
        assert_eq!(res.warnings.len(), 2, "{:?}", res.warnings);
        assert_eq!(res.skills[0].tags, vec!["x", "y"]);
        assert_eq!(res.skills[1].rel_path, PathBuf::from("cat/beta"));

        let agents = Library::open_kind(r.join("_AGENTS"), ItemKind::Agent).unwrap();
        let res = agents.scan().unwrap();
        assert_eq!(res.skills.len(), 1, "{:?}", res.warnings);
        assert_eq!(res.skills[0].id, "coder");
        assert_eq!(res.skills[0].kind, ItemKind::Agent);
        assert_eq!(res.skills[0].files, vec!["coder.md"]);
        assert!(res.warnings.is_empty(), "{:?}", res.warnings);
    }

    #[test]
    fn create_update_delete() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open(tmp.path()).unwrap();
        let s = lib
            .create("my-skill", "does things", Some("dev"), &["Rust".into()], &[])
            .unwrap();
        assert_eq!(s.tags, vec!["rust"]);
        assert_eq!(s.category.as_deref(), Some("dev"));
        let s = lib
            .update_meta(
                "my-skill",
                Some(&["a".into(), "b".into()]),
                Some(None),
                Some(&["codex".into()]),
                None,
            )
            .unwrap();
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
    fn agent_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let lib = Library::open_kind(tmp.path(), ItemKind::Agent).unwrap();
        let a = lib
            .create("coder", "writes code", Some("dev"), &["code".into()], &[])
            .unwrap();
        assert!(tmp.path().join("coder.md").is_file());
        assert_eq!(a.kind, ItemKind::Agent);
        assert!(lib.read_file("coder", "coder.md").unwrap().contains("name: coder"));
        assert!(lib.read_file("coder", "other.md").is_err());
        let a = lib
            .write_main("coder", "---\nname: coder\ndescription: v2\nmodel: opus\n---\nbody\n")
            .unwrap();
        assert_eq!(a.description, "v2");
        assert_eq!(a.extra.get("model").map(String::as_str), Some("opus"));
        let ext = tempfile::tempdir().unwrap();
        write(
            &ext.path().join("reviewer.md"),
            "---\nname: reviewer\ndescription: R\n---\nx",
        );
        let r = lib.import(&ext.path().join("reviewer.md"), None).unwrap();
        assert_eq!(r.id, "reviewer");
        lib.delete("coder").unwrap();
        assert!(lib.get("coder").is_err());
        assert_eq!(lib.scan().unwrap().skills.len(), 1);
    }

    #[test]
    fn safe_join_rejects_escape() {
        assert!(safe_join(Path::new("/x"), "../etc").is_err());
        assert!(safe_join(Path::new("/x"), "/etc").is_err());
        assert!(safe_join(Path::new("/x"), "a/b.md").is_ok());
    }
}
