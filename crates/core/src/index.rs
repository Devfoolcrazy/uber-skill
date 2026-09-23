//! `INDEX.md` at the library root: one line per skill and agent, generated from
//! the frontmatters so a reader (a person on GitHub, a model asked "which skill
//! does X?") finds an item without opening every `SKILL.md`.
//!
//! The index is a derived view, never a source: the engine rewrites it after
//! every change it makes to the library, and a library edited by hand is caught
//! by the lint and by the next scan. Its content depends only on the items, so
//! the file changes only when an item does and Git diffs stay meaningful.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::Result;
use crate::fsutil;
use crate::library::libraries;
use crate::lint::{Issue, Severity};
use crate::model::{ItemKind, Skill};

pub const INDEX_FILE: &str = "INDEX.md";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IndexStatus {
    pub path: PathBuf,
    pub exists: bool,
    /// The file matches what the library would generate now.
    pub fresh: bool,
}

/// Where the index lives: the library root.
pub fn path(cfg: &Config) -> Result<PathBuf> {
    Ok(cfg.library_path()?.join(INDEX_FILE))
}

/// The index text for the current state of the library.
pub fn render(cfg: &Config) -> Result<String> {
    let root = cfg.library_path()?;
    let mut sections = Vec::new();
    for lib in libraries(cfg)? {
        let mut items = lib.scan()?.skills;
        items.sort_by(|a, b| a.id.cmp(&b.id));
        let folder = lib.root().strip_prefix(&root).unwrap_or(lib.root()).to_path_buf();
        sections.push((lib.kind(), folder, items));
    }
    Ok(render_sections(&sections))
}

fn render_sections(sections: &[(ItemKind, PathBuf, Vec<Skill>)]) -> String {
    let mut out = String::new();
    out.push_str("# Index de la bibliothèque\n\n");
    out.push_str(
        "Généré par Uber Skill à partir du frontmatter de chaque élément ; ne pas modifier à la main. \
         Une ligne par élément : identifiant, catégorie, description, tags, harnais requis et chemin.\n",
    );
    for (kind, folder, items) in sections {
        let title = match kind {
            ItemKind::Skill => "Skills",
            ItemKind::Agent => "Agents",
        };
        out.push_str(&format!("\n## {title} ({})\n\n", items.len()));
        if items.is_empty() {
            out.push_str("Aucun.\n");
        }
        for item in items {
            out.push_str(&line(item, folder));
        }
    }
    out
}

fn line(item: &Skill, folder: &Path) -> String {
    let mut s = format!("- **{}**", item.id);
    if let Some(c) = &item.category {
        s.push_str(&format!(" [{c}]"));
    }
    s.push_str(" — ");
    s.push_str(&one_line(&item.description));
    if !item.tags.is_empty() {
        s.push_str(&format!(" — tags : {}", item.tags.join(", ")));
    }
    if !item.hosts.is_empty() {
        s.push_str(&format!(" — harnais : {}", item.hosts.join(", ")));
    }
    let rel = folder.join(&item.rel_path);
    s.push_str(&format!(" — `{}`\n", rel.to_string_lossy().replace('\\', "/")));
    s
}

fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Compare the file on disk with what the library would generate.
pub fn status(cfg: &Config) -> Result<IndexStatus> {
    let path = path(cfg)?;
    let expected = render(cfg)?;
    let current = if path.is_file() {
        Some(fsutil::read_to_string(&path)?)
    } else {
        None
    };
    Ok(IndexStatus {
        exists: current.is_some(),
        fresh: current.as_deref() == Some(expected.as_str()),
        path,
    })
}

/// Bring the index up to date. Writes only when the content differs, so an
/// unchanged library leaves the file (and its Git status) alone.
pub fn update(cfg: &Config) -> Result<IndexStatus> {
    let path = path(cfg)?;
    let expected = render(cfg)?;
    let current = if path.is_file() {
        Some(fsutil::read_to_string(&path)?)
    } else {
        None
    };
    if current.as_deref() != Some(expected.as_str()) {
        fsutil::write_string(&path, &expected)?;
    }
    Ok(IndexStatus {
        path,
        exists: true,
        fresh: true,
    })
}

/// The lint finding for a missing or stale index, if any.
pub fn lint(cfg: &Config) -> Result<Option<Issue>> {
    let status = status(cfg)?;
    Ok(if !status.exists {
        Some(Issue::index(
            "index-missing",
            "INDEX.md is missing: run `uber-skill index`",
        ))
    } else if !status.fresh {
        Some(Issue::index(
            "index-stale",
            "INDEX.md does not match the library: run `uber-skill index`",
        ))
    } else {
        None
    })
}

impl Issue {
    fn index(code: &'static str, message: &str) -> Issue {
        Issue::new(Severity::Warning, "index", code, vec![], message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use std::fs;
    use tempfile::{tempdir, TempDir};

    fn library() -> (TempDir, Config) {
        let tmp = tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::create_dir(root.join("skills")).unwrap();
        fs::create_dir(root.join("agents")).unwrap();
        let skills = Library::open(root.join("skills")).unwrap();
        skills
            .create(
                "review-pr",
                "Review a pull request\nline by line.",
                Some("review"),
                &["git".into(), "quality".into()],
                &["claude-code".into()],
            )
            .unwrap();
        skills
            .create("commit-message", "Write a commit message", None, &[], &[])
            .unwrap();
        let agents = Library::open_kind(root.join("agents"), ItemKind::Agent).unwrap();
        agents
            .create("reviewer", "Reviews code", Some("review"), &[], &[])
            .unwrap();
        let cfg = Config {
            library_path: Some(root),
            ..Config::default()
        };
        (tmp, cfg)
    }

    #[test]
    fn renders_one_sorted_line_per_item_with_paths_from_the_root() {
        let (_tmp, cfg) = library();
        let text = render(&cfg).unwrap();
        assert!(text.contains("## Skills (2)\n\n- **commit-message** — Write a commit message — `skills/commit-message`\n- **review-pr** [review] — Review a pull request line by line. — tags : git, quality — harnais : claude-code — `skills/review-pr`\n"), "{text}");
        assert!(
            text.contains("## Agents (1)\n\n- **reviewer** [review] — Reviews code — `agents/reviewer.md`\n"),
            "{text}"
        );
        assert_eq!(text, render(&cfg).unwrap());
    }

    #[test]
    fn update_writes_once_and_lint_catches_hand_edits() {
        let (_tmp, cfg) = library();
        assert_eq!(lint(&cfg).unwrap().map(|i| i.code), Some("index-missing"));
        let status = update(&cfg).unwrap();
        assert!(status.exists && status.fresh);
        assert!(lint(&cfg).unwrap().is_none());
        let written = fs::metadata(&status.path).unwrap().modified().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        update(&cfg).unwrap();
        assert_eq!(
            fs::metadata(&status.path).unwrap().modified().unwrap(),
            written,
            "unchanged library, untouched file"
        );

        Library::open(cfg.skills_path().unwrap())
            .unwrap()
            .update_meta("commit-message", None, Some(Some("writing")), None, None)
            .unwrap();
        assert_eq!(lint(&cfg).unwrap().map(|i| i.code), Some("index-stale"));
        assert!(!status_of(&cfg).fresh);
        update(&cfg).unwrap();
        assert!(fs::read_to_string(&status.path)
            .unwrap()
            .contains("- **commit-message** [writing]"));
        assert!(lint(&cfg).unwrap().is_none());
    }

    fn status_of(cfg: &Config) -> IndexStatus {
        status(cfg).unwrap()
    }

    #[test]
    fn a_library_without_agents_folder_still_indexes() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::create_dir(root.join("skills")).unwrap();
        let cfg = Config {
            library_path: Some(root.clone()),
            ..Config::default()
        };
        let text = render(&cfg).unwrap();
        assert!(text.contains("## Skills (0)\n\nAucun.\n"), "{text}");
        assert!(!text.contains("## Agents"));
        update(&cfg).unwrap();
        assert!(root.join(INDEX_FILE).is_file());
    }
}
