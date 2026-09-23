//! Suggest which skills and agents of the library fit a project, through the
//! Claude Code CLI in headless mode (`claude -p`), the same way [`crate::refine`]
//! works.
//!
//! Claude only ever sees what [`gather`] collects: the library index, a shallow
//! file tree of the project, the text of a few well-known root files (README,
//! CLAUDE.md, manifests) truncated, what the library already installed there,
//! and the answers the user typed. No source file is sent. The answer is a list
//! of ids checked against the library: anything Claude made up is dropped and
//! reported. Nothing is installed here; the caller decides.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{Error, RefineError, Result};
use crate::fsutil;
use crate::index;
use crate::install;
use crate::library::libraries;
use crate::model::ItemKind;
use crate::refine::{ask_claude, claude_path, unfenced};
use crate::targets::Target;

const SYSTEM_PROMPT: &str = "You help a developer pick, from their personal library of agent skills and agents, \
the ones worth installing in a software project. You receive the library index (one line per item: id, category, \
description, tags, required harness, path), a shallow file tree of the project, the text of a few root files, what \
is already installed there, and answers the developer gave about the project. \
Reply with a single JSON object and nothing else: no preamble, no code fence. Shape: \
{\"summary\": string, \"suggestions\": [{\"kind\": \"skill\"|\"agent\", \"id\": string, \"reason\": string, \
\"confidence\": \"high\"|\"medium\"|\"low\"}]}. \
Rules: use only ids that appear in the index, exactly as written; never invent one. Skip items already installed. \
Suggest between 3 and 10 items, the most useful first; fewer when the library has little that fits, never padding. \
Prefer items whose description matches what the project actually does over generic ones. An item that depends on a \
harness the project does not use gets at most medium confidence. Each reason is one concrete sentence tied to \
something you saw (a file, a manifest, an answer). Write the summary (two sentences at most, what kind of project \
this is) and the reasons in French.";

/// What the user typed about the project; every field is optional.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    /// Kind of project: game, desktop app, web site, library…
    #[serde(default)]
    pub kind: Option<String>,
    /// Languages, frameworks, tools.
    #[serde(default)]
    pub stack: Option<String>,
    /// What the user is about to do in it.
    #[serde(default)]
    pub goal: Option<String>,
}

impl Profile {
    fn is_empty(&self) -> bool {
        [&self.kind, &self.stack, &self.goal]
            .iter()
            .all(|f| f.as_deref().map(str::trim).unwrap_or_default().is_empty())
    }
}

/// Text of one root file sent to Claude, truncated.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Excerpt {
    /// Relative to the project.
    pub path: String,
    pub text: String,
    pub truncated: bool,
}

/// Everything about the project that leaves the machine, so the user can see it
/// before asking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Context {
    pub project: PathBuf,
    /// The project looks new: no file worth reading and an almost empty tree.
    pub empty: bool,
    /// Shallow file tree, one entry per line, relative paths.
    pub tree: Vec<String>,
    pub tree_truncated: bool,
    pub excerpts: Vec<Excerpt>,
    /// `kind:id` of what the library already installed there, across targets.
    pub installed: Vec<String>,
    /// Number of items in the library index sent along.
    pub catalogue_items: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suggestion {
    pub kind: ItemKind,
    pub id: String,
    pub reason: String,
    pub confidence: Confidence,
    /// Already installed in the project (Claude was told to skip these; kept, unchecked, when it did not).
    pub installed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Suggestions {
    pub summary: String,
    /// Most confident first, in Claude's order within a confidence level.
    pub suggestions: Vec<Suggestion>,
    /// Ids Claude proposed that the library does not have.
    pub unknown: Vec<String>,
}

/// Folders never worth listing: dependencies, build output, VCS internals.
const SKIPPED_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "build",
    "out",
    ".svelte-kit",
    ".next",
    ".nuxt",
    "__pycache__",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
    ".cache",
    "vendor",
    "Pods",
    "DerivedData",
];
/// Hidden folders that say something about the project anyway.
const SHOWN_HIDDEN: &[&str] = &[".claude", ".agents", ".cursor", ".github", ".codex"];
/// Root files whose text is worth sending, in the order they are listed.
const ROOT_FILES: &[&str] = &[
    "README.md",
    "README",
    "CLAUDE.md",
    "AGENTS.md",
    "CONTEXT.md",
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "requirements.txt",
    "go.mod",
    "Gemfile",
    "composer.json",
    "pubspec.yaml",
    "project.godot",
    "Makefile",
    "docker-compose.yml",
    "tauri.conf.json",
];
const MAX_TREE_ENTRIES: usize = 200;
const MAX_TREE_DEPTH: usize = 3;
const MAX_EXCERPT_CHARS: usize = 4000;

fn listed(entry: &walkdir::DirEntry, depth: usize) -> bool {
    let name = entry.file_name().to_str().unwrap_or("");
    if entry.file_type().is_dir() && SKIPPED_DIRS.contains(&name) {
        return false;
    }
    if name.starts_with('.') && !(depth == 1 && SHOWN_HIDDEN.contains(&name)) {
        return false;
    }
    true
}

fn tree(project: &Path) -> (Vec<String>, bool) {
    let mut entries = Vec::new();
    let mut truncated = false;
    let walker = walkdir::WalkDir::new(project)
        .min_depth(1)
        .max_depth(MAX_TREE_DEPTH)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| listed(e, e.depth()));
    for entry in walker.flatten() {
        if entries.len() >= MAX_TREE_ENTRIES {
            truncated = true;
            break;
        }
        let rel = entry
            .path()
            .strip_prefix(project)
            .unwrap_or(entry.path())
            .to_string_lossy()
            .replace('\\', "/");
        entries.push(if entry.file_type().is_dir() {
            format!("{rel}/")
        } else {
            rel
        });
    }
    (entries, truncated)
}

fn excerpts(project: &Path) -> Result<Vec<Excerpt>> {
    let mut out = Vec::new();
    for name in ROOT_FILES {
        let path = project.join(name);
        if !path.is_file() || !fsutil::is_text_file(&path) {
            continue;
        }
        let text = fsutil::read_to_string(&path)?;
        let truncated = text.chars().count() > MAX_EXCERPT_CHARS;
        let text = if truncated {
            text.chars().take(MAX_EXCERPT_CHARS).collect()
        } else {
            text
        };
        out.push(Excerpt {
            path: name.to_string(),
            text,
            truncated,
        });
    }
    Ok(out)
}

fn installed(cfg: &Config, project: &Path) -> Result<Vec<String>> {
    let mut out = Vec::new();
    let libs = libraries(cfg)?;
    for target in Target::ALL {
        for kind in [ItemKind::Skill, ItemKind::Agent] {
            let Ok(dir) = target.dir_for(kind, project) else {
                continue;
            };
            if !dir.join(install::LOCK_FILE).is_file() {
                continue;
            }
            let library = libs.iter().find(|l| l.kind() == kind);
            for item in install::status(library, project, &target, kind)? {
                let key = format!("{}:{}", kind.label(), item.id);
                if !out.contains(&key) {
                    out.push(key);
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Collect what would be sent for `project`. Reads the project and the library; sends nothing.
pub fn gather(cfg: &Config, project: &Path) -> Result<Context> {
    if !project.is_dir() {
        return Err(Error::NotADirectory(project.to_path_buf()));
    }
    let project = project.canonicalize().map_err(|e| Error::io(project, e))?;
    let (tree, tree_truncated) = tree(&project);
    let excerpts = excerpts(&project)?;
    let installed = installed(cfg, &project)?;
    let catalogue_items = libraries(cfg)?
        .iter()
        .map(|l| l.scan().map(|s| s.skills.len()))
        .sum::<Result<usize>>()?;
    Ok(Context {
        empty: excerpts.is_empty() && tree.len() < 3,
        project,
        tree,
        tree_truncated,
        excerpts,
        installed,
        catalogue_items,
    })
}

fn prompt(context: &Context, profile: &Profile, catalogue: &str) -> String {
    let mut p = String::new();
    p.push_str("# Project\n\n");
    p.push_str(&format!(
        "Folder name: {}\n",
        context
            .project
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
    ));
    if profile.is_empty() {
        p.push_str("The developer gave no answers about it; rely on the files.\n");
    } else {
        p.push_str("Answers from the developer:\n");
        for (label, value) in [
            ("Kind", &profile.kind),
            ("Stack", &profile.stack),
            ("Goal", &profile.goal),
        ] {
            if let Some(v) = value.as_deref().map(str::trim).filter(|v| !v.is_empty()) {
                p.push_str(&format!("- {label}: {v}\n"));
            }
        }
    }
    if context.empty {
        p.push_str("\nThe project folder is new or almost empty: judge from the answers and the folder name.\n");
    }
    p.push_str("\n## Already installed from the library (skip these)\n\n");
    if context.installed.is_empty() {
        p.push_str("Nothing.\n");
    }
    for item in &context.installed {
        p.push_str(&format!("- {item}\n"));
    }
    p.push_str("\n## File tree (shallow; dependencies and build output omitted)\n\n");
    for entry in &context.tree {
        p.push_str(&format!("{entry}\n"));
    }
    if context.tree_truncated {
        p.push_str("… (truncated)\n");
    }
    for excerpt in &context.excerpts {
        p.push_str(&format!(
            "\n## {}{}\n\n<<<\n{}\n>>>\n",
            excerpt.path,
            if excerpt.truncated { " (truncated)" } else { "" },
            excerpt.text
        ));
    }
    p.push_str("\n# Library index\n\n");
    p.push_str(catalogue);
    p
}

#[derive(Deserialize)]
struct Answer {
    #[serde(default)]
    summary: String,
    #[serde(default)]
    suggestions: Vec<AnswerItem>,
}

#[derive(Deserialize)]
struct AnswerItem {
    #[serde(default)]
    kind: Option<String>,
    id: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    confidence: Option<String>,
}

/// Keep only what the library really has, mark what is already installed, and
/// order by confidence.
fn checked(cfg: &Config, context: &Context, answer: &str) -> Result<Suggestions> {
    let invalid = |why: String| Error::Refine(RefineError::InvalidSuggestions(why));
    let parsed: Answer = serde_json::from_str(unfenced(answer)).map_err(|e| invalid(e.to_string()))?;
    let known: Vec<(ItemKind, String)> = libraries(cfg)?
        .iter()
        .map(|l| {
            l.scan()
                .map(|s| s.skills.into_iter().map(|i| (i.kind, i.id)).collect::<Vec<_>>())
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect();
    let mut suggestions: Vec<Suggestion> = Vec::new();
    let mut unknown = Vec::new();
    for item in parsed.suggestions {
        let id = item.id.trim().to_string();
        let kind = item
            .kind
            .as_deref()
            .and_then(ItemKind::parse)
            .filter(|k| known.contains(&(*k, id.clone())))
            .or_else(|| known.iter().find(|(_, i)| *i == id).map(|(k, _)| *k));
        let Some(kind) = kind else {
            if !unknown.contains(&id) {
                unknown.push(id);
            }
            continue;
        };
        if suggestions.iter().any(|s| s.kind == kind && s.id == id) {
            continue;
        }
        let confidence = match item.confidence.as_deref().map(str::to_ascii_lowercase).as_deref() {
            Some("high") => Confidence::High,
            Some("low") => Confidence::Low,
            _ => Confidence::Medium,
        };
        suggestions.push(Suggestion {
            installed: context.installed.contains(&format!("{}:{id}", kind.label())),
            kind,
            id,
            reason: item.reason.trim().to_string(),
            confidence,
        });
    }
    if suggestions.is_empty() && unknown.is_empty() && parsed.summary.trim().is_empty() {
        return Err(invalid("no suggestion in the answer".into()));
    }
    // A stable sort keeps Claude's order inside each confidence level.
    suggestions.sort_by_key(|s| std::cmp::Reverse(s.confidence));
    Ok(Suggestions {
        summary: parsed.summary.trim().to_string(),
        suggestions,
        unknown,
    })
}

/// Ask Claude which items fit `project`. Reads the project, sends [`gather`]'s
/// context and the library index, installs nothing.
pub fn suggest(cfg: &Config, project: &Path, profile: &Profile) -> Result<Suggestions> {
    let context = gather(cfg, project)?;
    let catalogue = index::render(cfg)?;
    let claude = claude_path()?;
    let answer = ask_claude(&claude, SYSTEM_PROMPT, &prompt(&context, profile, &catalogue))?;
    checked(cfg, &context, &answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    use std::fs;
    use tempfile::{tempdir, TempDir};

    fn setup() -> (TempDir, Config, PathBuf) {
        let tmp = tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        fs::create_dir_all(root.join("lib/skills")).unwrap();
        fs::create_dir_all(root.join("lib/agents")).unwrap();
        let skills = Library::open(root.join("lib/skills")).unwrap();
        skills
            .create("rust-tests", "Write Rust tests", Some("dev"), &["rust".into()], &[])
            .unwrap();
        skills
            .create("seo", "Optimise a web site for search", None, &[], &[])
            .unwrap();
        Library::open_kind(root.join("lib/agents"), ItemKind::Agent)
            .unwrap()
            .create("reviewer", "Reviews code", None, &[], &["claude-code".into()])
            .unwrap();
        let cfg = Config {
            library_path: Some(root.join("lib")),
            ..Config::default()
        };
        let project = root.join("game");
        fs::create_dir_all(project.join("src")).unwrap();
        fs::create_dir_all(project.join("node_modules/left-pad")).unwrap();
        fs::create_dir_all(project.join("target/debug")).unwrap();
        fs::create_dir_all(project.join(".claude")).unwrap();
        fs::write(project.join("Cargo.toml"), "[package]\nname = \"game\"\n").unwrap();
        fs::write(project.join("README.md"), "# Game\nA roguelike in Rust.\n").unwrap();
        fs::write(project.join("src/main.rs"), "fn main() { let secret = 42; }\n").unwrap();
        fs::write(project.join("node_modules/left-pad/index.js"), "x").unwrap();
        install::install(&skills.get("seo").unwrap(), &project, &Target::ClaudeCode).unwrap();
        (tmp, cfg, project)
    }

    #[test]
    fn gathers_a_shallow_tree_root_files_and_installed_items_only() {
        let (_tmp, cfg, project) = setup();
        let context = gather(&cfg, &project).unwrap();
        assert!(context.tree.contains(&"src/".to_string()) && context.tree.contains(&"src/main.rs".to_string()));
        assert!(context.tree.contains(&".claude/".to_string()), "{:?}", context.tree);
        assert!(
            !context
                .tree
                .iter()
                .any(|e| e.starts_with("node_modules") || e.starts_with("target")),
            "{:?}",
            context.tree
        );
        let paths: Vec<&str> = context.excerpts.iter().map(|e| e.path.as_str()).collect();
        assert_eq!(paths, ["README.md", "Cargo.toml"]);
        assert_eq!(context.installed, ["skill:seo"]);
        assert_eq!(context.catalogue_items, 3);
        assert!(!context.empty);

        let empty = project.parent().unwrap().join("fresh");
        fs::create_dir(&empty).unwrap();
        let context = gather(&cfg, &empty).unwrap();
        assert!(context.empty && context.tree.is_empty() && context.excerpts.is_empty());
    }

    #[test]
    fn keeps_known_ids_flags_installed_ones_and_reports_the_rest() {
        let (_tmp, cfg, project) = setup();
        let context = gather(&cfg, &project).unwrap();
        let answer = r#"```json
{"summary": "Un jeu en Rust.", "suggestions": [
  {"kind": "skill", "id": "seo", "reason": "déjà là", "confidence": "low"},
  {"kind": "agent", "id": "made-up", "reason": "n'existe pas", "confidence": "high"},
  {"id": "reviewer", "reason": "relecture", "confidence": "medium"},
  {"kind": "skill", "id": "rust-tests", "reason": "Cargo.toml", "confidence": "high"},
  {"kind": "skill", "id": "rust-tests", "reason": "doublon"}
]}
```"#;
        let result = checked(&cfg, &context, answer).unwrap();
        assert_eq!(result.summary, "Un jeu en Rust.");
        let got: Vec<(ItemKind, &str, Confidence, bool)> = result
            .suggestions
            .iter()
            .map(|s| (s.kind, s.id.as_str(), s.confidence, s.installed))
            .collect();
        assert_eq!(
            got,
            [
                (ItemKind::Skill, "rust-tests", Confidence::High, false),
                (ItemKind::Agent, "reviewer", Confidence::Medium, false),
                (ItemKind::Skill, "seo", Confidence::Low, true),
            ]
        );
        assert_eq!(result.unknown, ["made-up"]);
        assert!(matches!(
            checked(&cfg, &context, "Sure! Here are my picks."),
            Err(Error::Refine(RefineError::InvalidSuggestions(_)))
        ));
    }

    #[cfg(unix)]
    #[test]
    fn sends_the_index_and_root_files_but_no_source_code() {
        use std::os::unix::fs::PermissionsExt;
        let _env = crate::refine::tests::ENV.lock().unwrap_or_else(|e| e.into_inner());
        let (tmp, cfg, project) = setup();
        let cli = tmp.path().join("fake-claude");
        fs::write(
            &cli,
            r#"#!/bin/sh
cat > "$0.stdin"
printf '{"is_error":false,"result":"{\\"summary\\":\\"Jeu Rust\\",\\"suggestions\\":[{\\"kind\\":\\"skill\\",\\"id\\":\\"rust-tests\\",\\"reason\\":\\"Cargo.toml\\",\\"confidence\\":\\"high\\"}]}"}'
"#,
        )
        .unwrap();
        fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
        std::env::set_var(crate::refine::CLAUDE_ENV, &cli);
        let profile = Profile {
            kind: Some("jeu".into()),
            stack: None,
            goal: Some("  ".into()),
        };
        let result = suggest(&cfg, &project, &profile).unwrap();
        std::env::remove_var(crate::refine::CLAUDE_ENV);
        assert_eq!(result.suggestions.len(), 1);
        assert_eq!(result.suggestions[0].id, "rust-tests");
        let stdin = fs::read_to_string(tmp.path().join("fake-claude.stdin")).unwrap();
        assert!(stdin.contains("- Kind: jeu") && !stdin.contains("- Goal"));
        assert!(stdin.contains("A roguelike in Rust.") && stdin.contains("name = \"game\""));
        assert!(stdin.contains("**rust-tests** [dev]") && stdin.contains("**reviewer**"));
        assert!(stdin.contains("- skill:seo"));
        assert!(stdin.contains("src/main.rs\n") && !stdin.contains("let secret"));
        assert!(!stdin.contains("left-pad"));
    }
}
