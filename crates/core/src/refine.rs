//! Optional, assisted rewriting of a skill's `SKILL.md` through the Claude Code
//! CLI in headless mode (`claude -p`).
//!
//! Nothing is written here until the user accepts: [`propose`] returns the
//! proposed text and its diff, [`accept`] writes it only if the file is still the
//! one that was reviewed. Claude runs without any tool and only sees the text of
//! `SKILL.md`; scripts and other files of the skill are not sent. A proposal may
//! change the body and the `description`; every other frontmatter key is locked.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use similar::TextDiff;

use crate::error::{Error, RefineError, Result, Stale};
use crate::frontmatter::SkillDoc;
use crate::library::{main_file, Library};
use crate::model::{ItemKind, Skill};

/// Overrides the lookup of the `claude` executable.
pub const CLAUDE_ENV: &str = "UBER_SKILL_CLAUDE";
const TIMEOUT: Duration = Duration::from_secs(300);

const SYSTEM_PROMPT: &str = "You improve SKILL.md files: instructions an AI agent loads to perform a task. \
You receive a request and the current file. Reply with the complete improved file and nothing else: \
no preamble, no explanation, no code fence. Keep the YAML frontmatter; you may rewrite its `description` \
(it decides when the skill triggers) but leave every other key untouched. Keep the language of the file. \
Keep what already works; do not invent tools, files or facts the file does not mention.";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Proposal {
    pub id: String,
    /// Text the proposal was built from; [`accept`] refuses to overwrite anything else.
    pub original: String,
    pub proposed: String,
    /// Unified diff from `original` to `proposed`, empty when nothing changed.
    pub diff: String,
    /// Frontmatter keys Claude altered that were put back.
    pub restored_keys: Vec<String>,
}

/// Where the Claude Code CLI lives. A desktop app launched from the Finder gets
/// a minimal PATH, so the usual install locations are tried as well.
pub fn claude_path() -> Result<PathBuf> {
    if let Some(custom) = std::env::var_os(CLAUDE_ENV).map(PathBuf::from) {
        return if custom.is_file() {
            Ok(custom)
        } else {
            Err(Error::Refine(RefineError::ClaudeNotFound))
        };
    }
    let from_path = std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .unwrap_or_default();
    let home = directories::BaseDirs::new().map(|d| d.home_dir().to_path_buf());
    let usual = home
        .iter()
        .flat_map(|h| [h.join(".local/bin"), h.join(".claude/local"), h.join(".npm-global/bin")])
        .chain([PathBuf::from("/opt/homebrew/bin"), PathBuf::from("/usr/local/bin")]);
    from_path
        .into_iter()
        .chain(usual)
        .map(|dir| dir.join("claude"))
        .find(|candidate| candidate.is_file())
        .ok_or(Error::Refine(RefineError::ClaudeNotFound))
}

/// PATH for the CLI itself: an npm install of Claude Code needs `node`, which a
/// Finder-launched app does not have on its PATH.
fn search_path(claude: &Path) -> std::ffi::OsString {
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let extra = claude
        .parent()
        .map(Path::to_path_buf)
        .into_iter()
        .chain(["/opt/homebrew/bin", "/usr/local/bin"].map(PathBuf::from));
    std::env::join_paths(std::env::split_paths(&inherited).chain(extra)).unwrap_or(inherited)
}

#[derive(Deserialize)]
struct CliResult {
    #[serde(default)]
    is_error: bool,
    #[serde(default)]
    result: String,
}

/// Run `claude -p` with `prompt` on stdin, in an empty directory, without tools,
/// MCP servers or a saved session. Shared with [`crate::suggest`].
pub(crate) fn ask_claude(claude: &Path, system_prompt: &str, prompt: &str) -> Result<String> {
    let workdir = tempfile::tempdir().map_err(|e| Error::io("temporary directory", e))?;
    let failed = |what: String| Error::Refine(RefineError::ClaudeFailed(what));
    let mut child = Command::new(claude)
        .args([
            "-p",
            "--output-format",
            "json",
            "--tools",
            "",
            "--strict-mcp-config",
            "--no-session-persistence",
        ])
        .args(["--system-prompt", system_prompt])
        .env("PATH", search_path(claude))
        .current_dir(workdir.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| failed(e.to_string()))?;
    let mut stdin = child.stdin.take().expect("piped stdin");
    let prompt = prompt.to_owned();
    // Pipes are drained on their own threads so a large answer cannot block the child.
    let writer = std::thread::spawn(move || stdin.write_all(prompt.as_bytes()));
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            pipe.read_to_string(&mut text).map(|_| text)
        })
    };
    let stdout = drain(Box::new(child.stdout.take().expect("piped stdout")));
    let stderr = drain(Box::new(child.stderr.take().expect("piped stderr")));

    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait().map_err(|e| failed(e.to_string()))? {
            Some(status) => break status,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(Error::Refine(RefineError::Timeout));
            }
            None => std::thread::sleep(Duration::from_millis(100)),
        }
    };
    let _ = writer.join();
    let stdout = stdout.join().ok().and_then(|r| r.ok()).unwrap_or_default();
    let stderr = stderr.join().ok().and_then(|r| r.ok()).unwrap_or_default();

    // Errors such as a missing login come back as a JSON result with `is_error`.
    match serde_json::from_str::<CliResult>(stdout.trim()) {
        Ok(answer) if status.success() && !answer.is_error => Ok(answer.result),
        Ok(answer) => Err(failed(answer.result)),
        Err(_) => Err(failed(
            if stderr.trim().is_empty() { stdout } else { stderr }.trim().to_owned(),
        )),
    }
}

/// Drop a code fence wrapped around the whole answer despite the instructions.
pub(crate) fn unfenced(answer: &str) -> &str {
    let trimmed = answer.trim();
    let Some(rest) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let Some(inner) = rest.strip_suffix("```") else {
        return trimmed;
    };
    inner.split_once('\n').map(|(_, body)| body).unwrap_or(inner)
}

/// Build the text to propose: the original frontmatter with at most a new
/// `description`, and the body Claude wrote.
fn locked(original: &str, answer: &str, path: &Path) -> Result<(String, Vec<String>)> {
    let invalid = |why: &str| Error::Refine(RefineError::InvalidAnswer(why.to_owned()));
    let current = SkillDoc::parse(original, path)?;
    let answer = SkillDoc::parse(unfenced(answer), path).map_err(|e| invalid(&e.to_string()))?;
    if answer.body.trim().is_empty() {
        return Err(invalid("empty body"));
    }
    let mut restored: Vec<String> = Vec::new();
    for (key, value) in &current.front {
        let name = key.as_str().unwrap_or_default();
        if name != "description" && answer.front.get(key) != Some(value) {
            restored.push(name.to_owned());
        }
    }
    restored.extend(
        answer
            .front
            .keys()
            .filter(|k| !current.front.contains_key(*k))
            .filter_map(|k| k.as_str())
            .map(str::to_owned),
    );

    let description = answer
        .get_str("description")
        .map(|d| d.trim().to_owned())
        .filter(|d| !d.is_empty());
    let front_text = match description {
        Some(d) if Some(&d) != current.get_str("description").as_ref() => {
            current.text_with_description(original, &d)?
        }
        _ => original.to_owned(),
    };
    // Keep the frontmatter block as written, swap only the body.
    let kept = SkillDoc::parse(&front_text, path)?;
    let head = &front_text[..front_text.len() - kept.body.len()];
    let mut body = answer.body.clone();
    if !body.ends_with('\n') {
        body.push('\n');
    }
    Ok((format!("{head}{body}"), restored))
}

fn unified(id: &str, original: &str, proposed: &str) -> String {
    if original == proposed {
        return String::new();
    }
    let name = format!("{id}/SKILL.md");
    TextDiff::from_lines(original, proposed)
        .unified_diff()
        .context_radius(3)
        .header(&name, &name)
        .to_string()
}

fn prompt(instruction: &str, original: &str) -> String {
    format!(
        "Request:\n{}\n\nCurrent SKILL.md, between the markers:\n<<<SKILL.md\n{original}\nSKILL.md>>>\n",
        instruction.trim()
    )
}

/// Ask Claude for an improved `SKILL.md`. Nothing is written.
pub fn propose(library: &Library, id: &str, instruction: &str) -> Result<Proposal> {
    if library.kind() != ItemKind::Skill {
        return Err(Error::Unsupported("refining is limited to skills".into()));
    }
    if instruction.trim().is_empty() {
        return Err(Error::Refine(RefineError::EmptyInstruction));
    }
    let original = library.read_main(id)?;
    let claude = claude_path()?;
    let answer = ask_claude(&claude, SYSTEM_PROMPT, &prompt(instruction, &original))?;
    let path = main_file(&library.item_path(id)?, ItemKind::Skill);
    let (proposed, restored_keys) = locked(&original, &answer, &path)?;
    Ok(Proposal {
        id: id.to_owned(),
        diff: unified(id, &original, &proposed),
        original,
        proposed,
        restored_keys,
    })
}

/// Write an accepted proposal, unless the file changed since it was made. This is
/// a local save like any other: no commit, no push.
pub fn accept(library: &Library, proposal: &Proposal) -> Result<Skill> {
    if library.read_main(&proposal.id)? != proposal.original {
        return Err(Error::GitStale(Stale::Item(proposal.id.clone())));
    }
    library.write_main(&proposal.id, &proposal.proposed)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The `claude` executable is chosen through the process environment: tests
    /// that set it (here and in `suggest`) take this lock.
    pub(crate) static ENV: std::sync::Mutex<()> = std::sync::Mutex::new(());

    const ORIGINAL: &str = "---\nname: review-pr\n# keep\ndescription: Review a PR\nallowed-tools: Read, Grep\nmetadata:\n  tags: git\n---\n# review-pr\n\nOld body\n";

    fn p() -> PathBuf {
        "SKILL.md".into()
    }

    #[test]
    fn locks_everything_but_body_and_description() {
        let answer = "```markdown\n---\nname: renamed\ndescription: Review a pull request before merge\nallowed-tools: Bash\nmetadata:\n  tags: git, hacked\nmodel: opus\n---\n# review-pr\n\nNew body\n```";
        let (proposed, mut restored) = locked(ORIGINAL, answer, &p()).unwrap();
        assert_eq!(
            proposed,
            "---\nname: review-pr\n# keep\ndescription: Review a pull request before merge\nallowed-tools: Read, Grep\nmetadata:\n  tags: git\n---\n# review-pr\n\nNew body\n"
        );
        restored.sort();
        assert_eq!(restored, ["allowed-tools", "metadata", "model", "name"]);
    }

    #[test]
    fn unchanged_description_keeps_the_frontmatter_byte_for_byte() {
        let answer = "---\nname: review-pr\ndescription: Review a PR\n---\nNew body";
        let (proposed, _) = locked(ORIGINAL, answer, &p()).unwrap();
        assert_eq!(proposed, ORIGINAL.replace("# review-pr\n\nOld body\n", "New body\n"));
    }

    #[test]
    fn rejects_answers_that_are_not_a_skill_file() {
        for answer in [
            "Sure! Here is the file…",
            "---\nname: x\ndescription: d\n---\n   \n",
            "",
        ] {
            assert!(
                matches!(
                    locked(ORIGINAL, answer, &p()),
                    Err(Error::Refine(RefineError::InvalidAnswer(_)))
                ),
                "{answer}"
            );
        }
    }

    #[cfg(unix)]
    mod with_a_fake_cli {
        use super::super::*;
        use super::ENV;
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        fn library(script: &str) -> (tempfile::TempDir, Library) {
            let tmp = tempfile::tempdir().unwrap();
            let lib = Library::open(tmp.path()).unwrap();
            lib.create("review-pr", "Review a PR", None, &["git".into()], &[])
                .unwrap();
            let cli = tmp.path().join("fake-claude");
            fs::write(&cli, format!("#!/bin/sh\n{script}\n")).unwrap();
            fs::set_permissions(&cli, fs::Permissions::from_mode(0o755)).unwrap();
            std::env::set_var(CLAUDE_ENV, &cli);
            (tmp, lib)
        }

        #[test]
        fn proposes_without_writing_then_accepts_or_detects_a_changed_file() {
            let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
            // The fake CLI records how it was driven next to itself.
            let (tmp, lib) = library(
                r#"printf '%s\n' "$@" > "$0.args"; cat > "$0.stdin"; pwd > "$0.cwd"
printf '{"is_error":false,"result":"---\\nname: review-pr\\ndescription: Better description\\n---\\nNew body\\n"}'"#,
            );
            let before = lib.read_main("review-pr").unwrap();
            let proposal = propose(&lib, "review-pr", "Clarify").unwrap();
            assert_eq!(lib.read_main("review-pr").unwrap(), before);
            assert!(proposal.proposed.contains("description: Better description"));
            let recorded = |what: &str| fs::read_to_string(tmp.path().join(format!("fake-claude.{what}"))).unwrap();
            let args = recorded("args");
            assert!(args.starts_with("-p\n--output-format\njson\n--tools\n\n--strict-mcp-config\n--no-session-persistence\n--system-prompt\n"));
            let stdin = recorded("stdin");
            assert!(stdin.contains("Clarify") && stdin.contains(&before));
            // Run outside the library, so no project instructions leak into the request.
            assert!(!recorded("cwd").trim().starts_with(tmp.path().to_str().unwrap()));
            assert!(proposal.diff.contains("+description: Better description"));
            assert!(proposal.proposed.contains("tags: git"));

            let saved = accept(&lib, &proposal).unwrap();
            assert_eq!(saved.description, "Better description");
            assert_eq!(lib.read_main("review-pr").unwrap(), proposal.proposed);
            // The proposal was made from the previous text: accepting it again must not overwrite.
            assert!(matches!(accept(&lib, &proposal), Err(Error::GitStale(Stale::Item(_)))));
            std::env::remove_var(CLAUDE_ENV);
        }

        #[test]
        fn reports_cli_errors_and_a_missing_cli() {
            let _env = ENV.lock().unwrap_or_else(|e| e.into_inner());
            let (tmp, lib) =
                library(r#"printf '{"is_error":true,"result":"Invalid API key · Please run /login"}'; exit 1"#);
            match propose(&lib, "review-pr", "Clarify") {
                Err(Error::Refine(RefineError::ClaudeFailed(why))) => assert!(why.contains("/login")),
                other => panic!("{other:?}"),
            }
            assert!(matches!(
                propose(&lib, "review-pr", "  "),
                Err(Error::Refine(RefineError::EmptyInstruction))
            ));
            std::env::set_var(CLAUDE_ENV, tmp.path().join("missing"));
            assert!(matches!(
                propose(&lib, "review-pr", "Clarify"),
                Err(Error::Refine(RefineError::ClaudeNotFound))
            ));
            std::env::remove_var(CLAUDE_ENV);
        }
    }
}
