//! Installing items into a project and tracking drift between the library copy,
//! the installed copy and what was installed originally.
//!
//! A lock file (`.uber-skill.lock.json`) inside the target directory records, for each
//! installed item, the library path and content hash at install time.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::error::{Error, Result};
use crate::fsutil::{self, copy_item, hash_path, is_text_file, list_files};
use crate::library::{read_item, Library, SKILL_FILE};
use crate::model::{ItemKind, Skill};
use crate::targets::Target;

pub const LOCK_FILE: &str = ".uber-skill.lock.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockEntry {
    pub id: String,
    /// Absolute path of the library item it came from.
    pub source: PathBuf,
    /// Content hash at install time.
    pub hash: String,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LockFile {
    pub version: u32,
    #[serde(default)]
    pub skills: BTreeMap<String, LockEntry>,
}

impl LockFile {
    pub fn load(target_dir: &Path) -> Result<LockFile> {
        let path = target_dir.join(LOCK_FILE);
        if !path.is_file() {
            return Ok(LockFile { version: 1, skills: BTreeMap::new() });
        }
        let text = fsutil::read_to_string(&path)?;
        Ok(serde_json::from_str(&text)?)
    }

    pub fn save(&self, target_dir: &Path) -> Result<()> {
        let path = target_dir.join(LOCK_FILE);
        if self.skills.is_empty() && path.is_file() {
            return std::fs::remove_file(&path).map_err(|e| Error::io(&path, e));
        }
        if self.skills.is_empty() {
            return Ok(());
        }
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        fsutil::write_string(&path, &text)
    }
}

/// Relationship between the installed copy, the lock record and the library copy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum DriftState {
    /// Installed copy == lock == library.
    UpToDate,
    /// Library changed since install, project copy untouched.
    LibraryUpdated,
    /// Project copy edited since install, library unchanged.
    ProjectModified,
    /// Both changed.
    Conflict,
    /// Present in the project but not recorded in the lock file.
    Untracked,
    /// Recorded in the lock file but the directory is gone.
    Missing,
    /// Lock refers to a library item that no longer exists.
    SourceMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InstalledSkill {
    pub id: String,
    pub state: DriftState,
    pub path: PathBuf,
    pub lock: Option<LockEntry>,
    /// Current hash of the installed copy (None if missing).
    pub installed_hash: Option<String>,
    /// Current hash of the library copy (None if not in the library).
    pub library_hash: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileDiff {
    pub file: String,
    /// `added`, `removed`, `modified`, `binary`
    pub kind: String,
    pub unified: String,
}

/// Path of an installed item inside the target directory.
pub fn installed_path(target_dir: &Path, id: &str, kind: ItemKind) -> PathBuf {
    match kind {
        ItemKind::Skill => target_dir.join(id),
        ItemKind::Agent => target_dir.join(format!("{id}.md")),
    }
}

/// A human-readable warning when an item declares hosts that exclude `target`.
pub fn host_mismatch(skill: &Skill, target: &Target) -> Option<String> {
    if target.accepts_hosts(&skill.hosts) {
        None
    } else {
        Some(format!(
            "{} is declared for {} but the target is {}",
            skill.id,
            skill.hosts.join(", "),
            target.label()
        ))
    }
}

pub fn install(skill: &Skill, project_root: &Path, target: &Target) -> Result<LockEntry> {
    let target_dir = target.dir_for(skill.kind, project_root)?;
    std::fs::create_dir_all(&target_dir).map_err(|e| Error::io(&target_dir, e))?;
    let dst = installed_path(&target_dir, &skill.id, skill.kind);
    copy_item(&skill.path, &dst)?;
    let entry = LockEntry {
        id: skill.id.clone(),
        source: skill.path.clone(),
        hash: hash_path(&dst)?,
        installed_at: Utc::now().to_rfc3339(),
    };
    let mut lock = LockFile::load(&target_dir)?;
    lock.skills.insert(skill.id.clone(), entry.clone());
    lock.save(&target_dir)?;
    Ok(entry)
}

pub fn uninstall(id: &str, project_root: &Path, target: &Target, kind: ItemKind) -> Result<()> {
    let target_dir = target.dir_for(kind, project_root)?;
    let dst = installed_path(&target_dir, id, kind);
    if dst.is_dir() {
        std::fs::remove_dir_all(&dst).map_err(|e| Error::io(&dst, e))?;
    } else if dst.is_file() {
        std::fs::remove_file(&dst).map_err(|e| Error::io(&dst, e))?;
    }
    let mut lock = LockFile::load(&target_dir)?;
    lock.skills.remove(id);
    lock.save(&target_dir)
}

/// Items physically present in a target directory, by id.
fn present_items(target_dir: &Path, kind: ItemKind) -> Result<BTreeMap<String, PathBuf>> {
    let mut present = BTreeMap::new();
    if !target_dir.is_dir() {
        return Ok(present);
    }
    for entry in std::fs::read_dir(target_dir).map_err(|e| Error::io(target_dir, e))? {
        let entry = entry.map_err(|e| Error::io(target_dir, e))?;
        let p = entry.path();
        let id = match kind {
            ItemKind::Skill if p.is_dir() && p.join(SKILL_FILE).is_file() => p.file_name().and_then(|n| n.to_str()).map(String::from),
            ItemKind::Agent if p.is_file() && p.extension().map(|e| e == "md").unwrap_or(false) => {
                p.file_stem().and_then(|n| n.to_str()).map(String::from)
            }
            _ => None,
        };
        if let Some(id) = id {
            present.insert(id, p);
        }
    }
    Ok(present)
}

/// Status of every item of `kind` in the project's target dir (tracked or not).
pub fn status(library: Option<&Library>, project_root: &Path, target: &Target, kind: ItemKind) -> Result<Vec<InstalledSkill>> {
    let Ok(target_dir) = target.dir_for(kind, project_root) else { return Ok(Vec::new()) };
    let lock = LockFile::load(&target_dir)?;
    let present = present_items(&target_dir, kind)?;
    let mut out = Vec::new();

    let mut ids: Vec<String> = present.keys().cloned().collect();
    ids.extend(lock.skills.keys().cloned());
    ids.sort();
    ids.dedup();

    for id in ids {
        let path = installed_path(&target_dir, &id, kind);
        let lock_entry = lock.skills.get(&id).cloned();
        let installed_hash = present.get(&id).map(|p| hash_path(p)).transpose()?;
        let description = present.get(&id).and_then(|p| read_item(p, None, kind).ok()).map(|s| s.description);
        let library_skill = match (library, &lock_entry) {
            (Some(lib), _) => lib.get(&id).ok(),
            (None, Some(e)) if e.source.exists() => read_item(&e.source, None, kind).ok(),
            _ => None,
        };
        let library_hash = library_skill.as_ref().map(|s| s.hash.clone());

        let state = match (&lock_entry, &installed_hash, &library_hash) {
            (None, Some(_), _) => DriftState::Untracked,
            (Some(_), None, _) => DriftState::Missing,
            (Some(_), Some(_), None) => DriftState::SourceMissing,
            (Some(l), Some(inst), Some(libh)) => {
                let lib_changed = libh != &l.hash;
                let proj_changed = inst != &l.hash;
                match (lib_changed, proj_changed) {
                    (false, false) => DriftState::UpToDate,
                    (true, false) => DriftState::LibraryUpdated,
                    (false, true) => DriftState::ProjectModified,
                    (true, true) => {
                        if inst == libh {
                            DriftState::UpToDate
                        } else {
                            DriftState::Conflict
                        }
                    }
                }
            }
            (None, None, _) => continue,
        };
        out.push(InstalledSkill { id, state, path, lock: lock_entry, installed_hash, library_hash, description });
    }
    Ok(out)
}

fn diff_text(name: &str, ta: &str, tb: &str, in_a: bool, in_b: bool) -> FileDiff {
    let kind = match (in_a, in_b) {
        (false, true) => "added",
        (true, false) => "removed",
        _ => "modified",
    };
    let unified = TextDiff::from_lines(ta, tb)
        .unified_diff()
        .context_radius(3)
        .header(&format!("a/{name}"), &format!("b/{name}"))
        .to_string();
    FileDiff { file: name.to_string(), kind: kind.into(), unified }
}

/// Unified diff of every differing file between two items (`a` = base, `b` = new).
/// Works for two directories or two files.
pub fn diff_paths(a: &Path, b: &Path) -> Result<Vec<FileDiff>> {
    if a.is_file() || b.is_file() {
        let name = b.file_name().or_else(|| a.file_name()).map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let (in_a, in_b) = (a.is_file(), b.is_file());
        let ta = if in_a { fsutil::read_to_string(a)? } else { String::new() };
        let tb = if in_b { fsutil::read_to_string(b)? } else { String::new() };
        if in_a && in_b && ta == tb {
            return Ok(Vec::new());
        }
        return Ok(vec![diff_text(&name, &ta, &tb, in_a, in_b)]);
    }
    let files_a = list_files(a)?;
    let files_b = list_files(b)?;
    let mut all: Vec<PathBuf> = files_a.iter().chain(files_b.iter()).cloned().collect();
    all.sort();
    all.dedup();
    let mut out = Vec::new();
    for rel in all {
        let pa = a.join(&rel);
        let pb = b.join(&rel);
        let name = rel.to_string_lossy().replace('\\', "/");
        let (in_a, in_b) = (pa.is_file(), pb.is_file());
        if !is_text_file(&rel) {
            let same = in_a && in_b && std::fs::read(&pa).ok() == std::fs::read(&pb).ok();
            if !same {
                out.push(FileDiff { file: name, kind: "binary".into(), unified: String::new() });
            }
            continue;
        }
        let ta = if in_a { fsutil::read_to_string(&pa).unwrap_or_default() } else { String::new() };
        let tb = if in_b { fsutil::read_to_string(&pb).unwrap_or_default() } else { String::new() };
        if in_a && in_b && ta == tb {
            continue;
        }
        out.push(diff_text(&name, &ta, &tb, in_a, in_b));
    }
    Ok(out)
}

/// Diff between the library copy (base) and the installed copy.
pub fn diff_installed(library: &Library, id: &str, project_root: &Path, target: &Target) -> Result<Vec<FileDiff>> {
    let lib_path = library.item_path(id)?;
    let inst = installed_path(&target.dir_for(library.kind(), project_root)?, id, library.kind());
    if !inst.exists() {
        return Err(Error::SkillNotFound(format!("{id} (not installed in {})", inst.display())));
    }
    diff_paths(&lib_path, &inst)
}

/// Re-copy the library version into the project (overwrites local edits).
pub fn sync_to_project(library: &Library, id: &str, project_root: &Path, target: &Target) -> Result<LockEntry> {
    let skill = library.get(id)?;
    install(&skill, project_root, target)
}

/// Copy the installed version back into the library and refresh the lock.
pub fn sync_to_library(library: &Library, id: &str, project_root: &Path, target: &Target) -> Result<Skill> {
    let kind = library.kind();
    let target_dir = target.dir_for(kind, project_root)?;
    let inst = installed_path(&target_dir, id, kind);
    if !inst.exists() {
        return Err(Error::SkillNotFound(format!("{id} (not installed in {})", inst.display())));
    }
    let lib_path = match library.item_path(id) {
        Ok(p) => p,
        Err(Error::SkillNotFound(_)) => installed_path(library.root(), id, kind),
        Err(e) => return Err(e),
    };
    copy_item(&inst, &lib_path)?;
    let skill = read_item(&lib_path, Some(library.root()), kind)?;
    let mut lock = LockFile::load(&target_dir)?;
    lock.skills.insert(
        id.to_string(),
        LockEntry { id: id.to_string(), source: lib_path, hash: skill.hash.clone(), installed_at: Utc::now().to_rfc3339() },
    );
    lock.save(&target_dir)?;
    Ok(skill)
}

/// Record an untracked installed item as coming from the library item with the same id.
pub fn adopt(library: &Library, id: &str, project_root: &Path, target: &Target) -> Result<LockEntry> {
    let kind = library.kind();
    let target_dir = target.dir_for(kind, project_root)?;
    let inst = installed_path(&target_dir, id, kind);
    let skill = library.get(id)?;
    let entry = LockEntry { id: id.to_string(), source: skill.path, hash: hash_path(&inst)?, installed_at: Utc::now().to_rfc3339() };
    let mut lock = LockFile::load(&target_dir)?;
    lock.skills.insert(id.to_string(), entry.clone());
    lock.save(&target_dir)?;
    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(p: &Path, s: &str) {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, s).unwrap();
    }

    #[test]
    fn install_status_drift_cycle() {
        let lib_tmp = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write(&lib_tmp.path().join("s1/SKILL.md"), "---\nname: s1\ndescription: one\n---\nv1\n");
        write(&lib_tmp.path().join("s1/scripts/run.sh"), "echo hi\n");
        let lib = Library::open(lib_tmp.path()).unwrap();
        let t = Target::ClaudeCode;
        let k = ItemKind::Skill;

        let skill = lib.get("s1").unwrap();
        install(&skill, proj.path(), &t).unwrap();
        assert!(proj.path().join(".claude/skills/s1/scripts/run.sh").is_file());
        assert!(proj.path().join(".claude/skills").join(LOCK_FILE).is_file());

        let st = status(Some(&lib), proj.path(), &t, k).unwrap();
        assert_eq!(st.len(), 1);
        assert_eq!(st[0].state, DriftState::UpToDate);

        // Edit in the library -> LibraryUpdated
        write(&lib.root().join("s1/SKILL.md"), "---\nname: s1\ndescription: one\n---\nv2\n");
        let st = status(Some(&lib), proj.path(), &t, k).unwrap();
        assert_eq!(st[0].state, DriftState::LibraryUpdated);
        let d = diff_installed(&lib, "s1", proj.path(), &t).unwrap();
        assert_eq!(d.len(), 1);
        assert!(d[0].unified.contains("-v2") && d[0].unified.contains("+v1"));

        // Edit in the project too -> Conflict
        write(&proj.path().join(".claude/skills/s1/SKILL.md"), "---\nname: s1\ndescription: one\n---\nv3\n");
        assert_eq!(status(Some(&lib), proj.path(), &t, k).unwrap()[0].state, DriftState::Conflict);

        // Push project version to library -> UpToDate
        let s = sync_to_library(&lib, "s1", proj.path(), &t).unwrap();
        assert!(s.body_chars > 0);
        assert_eq!(status(Some(&lib), proj.path(), &t, k).unwrap()[0].state, DriftState::UpToDate);
        assert!(lib.read_main("s1").unwrap().contains("v3"));

        // Edit project only -> ProjectModified, then pull library -> UpToDate
        write(&proj.path().join(".claude/skills/s1/SKILL.md"), "---\nname: s1\ndescription: one\n---\nv4\n");
        assert_eq!(status(Some(&lib), proj.path(), &t, k).unwrap()[0].state, DriftState::ProjectModified);
        sync_to_project(&lib, "s1", proj.path(), &t).unwrap();
        assert_eq!(status(Some(&lib), proj.path(), &t, k).unwrap()[0].state, DriftState::UpToDate);

        // Untracked + adopt
        write(&proj.path().join(".claude/skills/s2/SKILL.md"), "---\nname: s2\ndescription: two\n---\n");
        let st = status(Some(&lib), proj.path(), &t, k).unwrap();
        assert_eq!(st.iter().find(|s| s.id == "s2").unwrap().state, DriftState::Untracked);

        uninstall("s1", proj.path(), &t, k).unwrap();
        assert!(!proj.path().join(".claude/skills/s1").exists());
        let st = status(Some(&lib), proj.path(), &t, k).unwrap();
        assert_eq!(st.len(), 1);
    }

    #[test]
    fn agent_install_cycle() {
        let lib_tmp = tempfile::tempdir().unwrap();
        let proj = tempfile::tempdir().unwrap();
        write(&lib_tmp.path().join("coder.md"), "---\nname: coder\ndescription: C\nmodel: opus\n---\nv1\n");
        let lib = Library::open_kind(lib_tmp.path(), ItemKind::Agent).unwrap();
        let t = Target::ClaudeCode;
        let k = ItemKind::Agent;

        let a = lib.get("coder").unwrap();
        install(&a, proj.path(), &t).unwrap();
        assert!(proj.path().join(".claude/agents/coder.md").is_file());
        assert!(proj.path().join(".claude/agents").join(LOCK_FILE).is_file());
        assert_eq!(status(Some(&lib), proj.path(), &t, k).unwrap()[0].state, DriftState::UpToDate);

        // Agents are unsupported on other hosts.
        assert!(install(&a, proj.path(), &Target::Cursor).is_err());
        assert!(status(Some(&lib), proj.path(), &Target::Cursor, k).unwrap().is_empty());

        write(&proj.path().join(".claude/agents/coder.md"), "---\nname: coder\ndescription: C\nmodel: opus\n---\nv2\n");
        assert_eq!(status(Some(&lib), proj.path(), &t, k).unwrap()[0].state, DriftState::ProjectModified);
        let d = diff_installed(&lib, "coder", proj.path(), &t).unwrap();
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].file, "coder.md");
        sync_to_library(&lib, "coder", proj.path(), &t).unwrap();
        assert!(lib.read_main("coder").unwrap().contains("v2"));
        assert_eq!(status(Some(&lib), proj.path(), &t, k).unwrap()[0].state, DriftState::UpToDate);

        uninstall("coder", proj.path(), &t, k).unwrap();
        assert!(!proj.path().join(".claude/agents/coder.md").exists());
        assert!(status(Some(&lib), proj.path(), &t, k).unwrap().is_empty());
    }
}
