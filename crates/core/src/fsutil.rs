//! Filesystem helpers shared by hashing, copying and scanning.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::error::{Error, Result};

/// Directory or file names that are never part of a skill's content.
pub const IGNORED_NAMES: &[&str] = &[
    ".git",
    ".DS_Store",
    "node_modules",
    "__pycache__",
    ".venv",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    "target",
];

pub fn is_ignored(name: &str) -> bool {
    IGNORED_NAMES.contains(&name)
}

/// All files under `root`, as paths relative to `root`, sorted, ignoring [`IGNORED_NAMES`].
pub fn list_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let walker = WalkDir::new(root).follow_links(true).min_depth(1).into_iter().filter_entry(|e| {
        !e.file_name().to_str().map(is_ignored).unwrap_or(false)
    });
    for entry in walker {
        let entry = entry.map_err(|e| Error::io(root, e.into()))?;
        if entry.file_type().is_file() {
            let rel = entry
                .path()
                .strip_prefix(root)
                .expect("walkdir yields paths under root")
                .to_path_buf();
            files.push(rel);
        }
    }
    files.sort();
    Ok(files)
}

/// Content hash of a directory: sha256 over (relative path, size, bytes) of every file.
pub fn hash_dir(root: &Path) -> Result<String> {
    let mut hasher = Sha256::new();
    for rel in list_files(root)? {
        let abs = root.join(&rel);
        let bytes = std::fs::read(&abs).map_err(|e| Error::io(&abs, e))?;
        hasher.update(rel.to_string_lossy().as_bytes());
        hasher.update([0u8]);
        hasher.update((bytes.len() as u64).to_le_bytes());
        hasher.update(&bytes);
        hasher.update([0u8]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Copy a skill directory, skipping ignored entries. `dst` is created (and emptied first).
pub fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    if dst.exists() {
        std::fs::remove_dir_all(dst).map_err(|e| Error::io(dst, e))?;
    }
    std::fs::create_dir_all(dst).map_err(|e| Error::io(dst, e))?;
    for rel in list_files(src)? {
        let from = src.join(&rel);
        let to = dst.join(&rel);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
        }
        std::fs::copy(&from, &to).map_err(|e| Error::io(&from, e))?;
    }
    Ok(())
}

pub fn read_to_string(path: &Path) -> Result<String> {
    std::fs::read_to_string(path).map_err(|e| Error::io(path, e))
}

pub fn write_string(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| Error::io(parent, e))?;
    }
    std::fs::write(path, content).map_err(|e| Error::io(path, e))
}

/// Whether a path looks like a text file we can diff/edit.
pub fn is_text_file(path: &Path) -> bool {
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "md" | "txt" | "json" | "yaml" | "yml" | "toml" | "py" | "sh" | "js" | "ts"
                | "rs" | "csv" | "html" | "css" | "xml" | "mjs" | "cjs" | "svelte" | "sql"
        ),
        None => true,
    }
}
