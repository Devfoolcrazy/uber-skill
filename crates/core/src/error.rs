//! Errors carry a stable code and data, never user-facing prose: each front end
//! (desktop app, CLI) owns its wording and language. `Display` is for logs and
//! the CLI.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Why a repository cannot be published or updated until an external Git tool
/// has dealt with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case")]
pub enum BlockReason {
    #[error("HEAD is detached")]
    DetachedHead,
    #[error("the local branch has no commit")]
    NoCommits,
    #[error("the branch has no usable upstream")]
    NoUpstream,
    #[error("the remote is missing or has several push URLs")]
    AmbiguousPushUrl,
    #[error("a merge, rebase, cherry-pick or revert is in progress")]
    OperationInProgress,
    #[error("unresolved conflicts")]
    Conflicts,
    #[error("local and remote histories have diverged")]
    Diverged,
    #[error("submodule changes must be published with Git")]
    SubmoduleChange,
    #[error("the remote could not be reached, freshness is unverified")]
    Unverified,
}

/// What Git was doing when it failed; the stderr travels alongside.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum GitAction {
    #[error("git failed")]
    Run,
    #[error("clone failed")]
    Clone,
    #[error("commit failed, the selected files stay staged and nothing was pushed")]
    Commit,
    #[error("fast-forward failed, local changes were left untouched")]
    FastForward,
    #[error("unexpected git output")]
    Parse,
}

/// What changed between a reviewed preview or check and the action relying on it.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Stale {
    #[error("the working tree changed since the preview")]
    Preview,
    #[error("the repository state changed since the check")]
    Repository,
    #[error("the configured library changed since the check")]
    Library,
    #[error("{0} changed since the check")]
    Item(String),
}

/// A caller-supplied value rejected before anything ran.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum InputError {
    #[error("not the root of a Git working tree, the root is {0}")]
    NotRepositoryRoot(PathBuf),
    #[error("invalid repository URL")]
    CloneUrl,
    #[error("invalid folder name")]
    CloneName,
    #[error("cannot create {path}: {reason}")]
    CloneDestination { path: PathBuf, reason: String },
    #[error("nothing selected")]
    EmptySelection,
    #[error("the selection holds a file that is not in the preview")]
    UnknownSelection,
    #[error("empty commit message")]
    EmptyCommitMessage,
    #[error("no file selected and no local commit to push")]
    NothingToPublish,
    #[error("invalid category or tag: {0:?}")]
    RegistryValue(String),
    #[error("{0} is still used: replace it or strip it from the items using it")]
    RegistryValueInUse(String),
}

/// Why an assisted rewrite through the Claude Code CLI did not produce a proposal.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RefineError {
    #[error("the `claude` command was not found")]
    ClaudeNotFound,
    #[error("claude failed: {0}")]
    ClaudeFailed(String),
    #[error("claude did not answer in time")]
    Timeout,
    #[error("claude did not return a valid SKILL.md: {0}")]
    InvalidAnswer(String),
    #[error("empty request")]
    EmptyInstruction,
    #[error("claude did not return a list of suggestions: {0}")]
    InvalidSuggestions(String),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid frontmatter in {path}: {reason}")]
    Frontmatter { path: PathBuf, reason: String },
    #[error("skill not found: {0}")]
    SkillNotFound(String),
    #[error("skill already exists: {0}")]
    SkillExists(String),
    #[error("invalid skill id: {0}")]
    InvalidId(String),
    #[error("library path is not configured")]
    NoLibrary,
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Git could not be started, or a previous Git operation panicked.
    #[error("git is unavailable: {0}")]
    GitUnavailable(String),
    #[error("{action}: {stderr}")]
    GitCommand { action: GitAction, stderr: String },
    #[error("{0}")]
    GitStale(Stale),
    #[error("{0}")]
    GitBlocked(BlockReason),
    #[error("{0}")]
    InvalidInput(InputError),
    #[error("{0}")]
    Refine(RefineError),
    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    /// Name what Git was doing when a plain command failure came back.
    pub fn during(self, action: GitAction) -> Self {
        match self {
            Error::GitCommand { stderr, .. } => Error::GitCommand { action, stderr },
            other => other,
        }
    }

    /// Stable identifier a front end maps to its own wording.
    pub fn code(&self) -> &'static str {
        match self {
            Error::Io { .. } => "io",
            Error::Frontmatter { .. } => "frontmatter",
            Error::SkillNotFound(_) => "not-found",
            Error::SkillExists(_) => "already-exists",
            Error::InvalidId(_) => "invalid-id",
            Error::NoLibrary => "no-library",
            Error::Unsupported(_) => "unsupported",
            Error::GitUnavailable(_) => "git-unavailable",
            Error::GitCommand { action, .. } => match action {
                GitAction::Run => "git-failed",
                GitAction::Clone => "git-clone-failed",
                GitAction::Commit => "git-commit-failed",
                GitAction::FastForward => "git-fast-forward-failed",
                GitAction::Parse => "git-unexpected-output",
            },
            Error::GitStale(_) => "git-stale",
            Error::GitBlocked(reason) => match reason {
                BlockReason::DetachedHead => "blocked.detached-head",
                BlockReason::NoCommits => "blocked.no-commits",
                BlockReason::NoUpstream => "blocked.no-upstream",
                BlockReason::AmbiguousPushUrl => "blocked.ambiguous-push-url",
                BlockReason::OperationInProgress => "blocked.operation-in-progress",
                BlockReason::Conflicts => "blocked.conflicts",
                BlockReason::Diverged => "blocked.diverged",
                BlockReason::SubmoduleChange => "blocked.submodule-change",
                BlockReason::Unverified => "blocked.unverified",
            },
            Error::InvalidInput(input) => match input {
                InputError::NotRepositoryRoot(_) => "input.not-repository-root",
                InputError::CloneUrl => "input.clone-url",
                InputError::CloneName => "input.clone-name",
                InputError::CloneDestination { .. } => "input.clone-destination",
                InputError::EmptySelection => "input.empty-selection",
                InputError::UnknownSelection => "input.unknown-selection",
                InputError::EmptyCommitMessage => "input.empty-commit-message",
                InputError::NothingToPublish => "input.nothing-to-publish",
                InputError::RegistryValue(_) => "input.registry-value",
                InputError::RegistryValueInUse(_) => "input.registry-value-in-use",
            },
            Error::Refine(refine) => match refine {
                RefineError::ClaudeNotFound => "refine.claude-not-found",
                RefineError::ClaudeFailed(_) => "refine.claude-failed",
                RefineError::Timeout => "refine.timeout",
                RefineError::InvalidAnswer(_) => "refine.invalid-answer",
                RefineError::EmptyInstruction => "refine.empty-instruction",
                RefineError::InvalidSuggestions(_) => "suggest.invalid-answer",
            },
            Error::NotADirectory(_) => "not-a-directory",
            Error::Json(_) => "json",
            Error::Yaml(_) => "yaml",
        }
    }

    /// The variable part worth showing next to a translated message: a path,
    /// an id, Git's own stderr. Never translated.
    pub fn details(&self) -> Option<String> {
        match self {
            Error::Io { path, source } => Some(format!("{} : {source}", path.display())),
            Error::Frontmatter { path, reason } => Some(format!("{} : {reason}", path.display())),
            Error::SkillNotFound(id) | Error::SkillExists(id) | Error::InvalidId(id) => Some(id.clone()),
            Error::Unsupported(what) | Error::GitUnavailable(what) => Some(what.clone()),
            Error::GitCommand { stderr, .. } => Some(stderr.clone()).filter(|s| !s.is_empty()),
            Error::GitStale(Stale::Item(id)) => Some(id.clone()),
            Error::InvalidInput(InputError::NotRepositoryRoot(root)) => Some(root.display().to_string()),
            Error::InvalidInput(InputError::CloneDestination { path, reason }) => {
                Some(format!("{} : {reason}", path.display()))
            }
            Error::InvalidInput(InputError::RegistryValue(value) | InputError::RegistryValueInUse(value)) => {
                Some(value.clone())
            }
            Error::Refine(
                RefineError::ClaudeFailed(why) | RefineError::InvalidAnswer(why) | RefineError::InvalidSuggestions(why),
            ) => Some(why.clone()).filter(|w| !w.is_empty()),
            Error::Refine(_) => None,
            Error::NotADirectory(path) => Some(path.display().to_string()),
            Error::Json(e) => Some(e.to_string()),
            Error::Yaml(e) => Some(e.to_string()),
            Error::NoLibrary | Error::GitStale(_) | Error::GitBlocked(_) | Error::InvalidInput(_) => None,
        }
    }
}

/// Serializable form of an error, for front ends and for failures reported
/// inside an otherwise successful result (a fetch or push that did not go through).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ErrorPayload {
    pub code: String,
    /// English, for logs and as a fallback when the code is unknown.
    pub message: String,
    pub details: Option<String>,
}

impl ErrorPayload {
    pub fn new(code: &str, message: impl Into<String>, details: Option<String>) -> Self {
        ErrorPayload {
            code: code.into(),
            message: message.into(),
            details,
        }
    }
}

impl From<&Error> for ErrorPayload {
    fn from(e: &Error) -> Self {
        ErrorPayload::new(e.code(), e.to_string(), e.details())
    }
}

impl From<Error> for ErrorPayload {
    fn from(e: Error) -> Self {
        (&e).into()
    }
}

impl std::fmt::Display for ErrorPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<Error> {
        let io = || std::io::Error::other("denied");
        let mut all = vec![
            Error::io("/lib", io()),
            Error::Frontmatter {
                path: "/lib/a".into(),
                reason: "bad".into(),
            },
            Error::SkillNotFound("a".into()),
            Error::SkillExists("a".into()),
            Error::InvalidId("a b".into()),
            Error::NoLibrary,
            Error::Unsupported("Cursor does not support agents".into()),
            Error::GitUnavailable("not found".into()),
            Error::GitStale(Stale::Item("a".into())),
            Error::NotADirectory("/lib/file".into()),
            Error::Json(serde_json::from_str::<u8>("x").unwrap_err()),
            Error::Yaml(serde_yaml::from_str::<u8>("[").unwrap_err()),
        ];
        for action in [
            GitAction::Run,
            GitAction::Clone,
            GitAction::Commit,
            GitAction::FastForward,
            GitAction::Parse,
        ] {
            all.push(Error::GitCommand {
                action,
                stderr: "fatal: nope".into(),
            });
        }
        for input in [
            InputError::NotRepositoryRoot("/repo".into()),
            InputError::CloneUrl,
            InputError::CloneName,
            InputError::CloneDestination {
                path: "/x".into(),
                reason: "exists".into(),
            },
            InputError::EmptySelection,
            InputError::UnknownSelection,
            InputError::EmptyCommitMessage,
            InputError::NothingToPublish,
            InputError::RegistryValue("a, b".into()),
            InputError::RegistryValueInUse("git".into()),
        ] {
            all.push(Error::InvalidInput(input));
        }
        for refine in [
            RefineError::ClaudeNotFound,
            RefineError::ClaudeFailed("please run /login".into()),
            RefineError::Timeout,
            RefineError::InvalidAnswer("no frontmatter".into()),
            RefineError::EmptyInstruction,
            RefineError::InvalidSuggestions("not json".into()),
        ] {
            all.push(Error::Refine(refine));
        }
        all
    }

    const REASONS: [BlockReason; 9] = [
        BlockReason::DetachedHead,
        BlockReason::NoCommits,
        BlockReason::NoUpstream,
        BlockReason::AmbiguousPushUrl,
        BlockReason::OperationInProgress,
        BlockReason::Conflicts,
        BlockReason::Diverged,
        BlockReason::SubmoduleChange,
        BlockReason::Unverified,
    ];

    #[test]
    fn payload_keeps_git_output_as_details_and_context_as_code() {
        let failed = Error::GitCommand {
            action: GitAction::Run,
            stderr: "fatal: nope".into(),
        };
        let payload = ErrorPayload::from(failed.during(GitAction::Commit));
        assert_eq!(payload.code, "git-commit-failed");
        assert_eq!(payload.details.as_deref(), Some("fatal: nope"));
        assert!(payload.message.ends_with("fatal: nope"));
        assert_eq!(Error::NoLibrary.during(GitAction::Clone).code(), "no-library");
    }

    #[test]
    fn block_codes_match_their_serialized_reason() {
        for reason in REASONS {
            let name = serde_json::to_value(reason).unwrap();
            assert_eq!(
                Error::GitBlocked(reason).code(),
                format!("blocked.{}", name.as_str().unwrap())
            );
        }
    }

    /// The desktop app owns the wording: a code without an entry there would
    /// surface in English.
    #[test]
    fn desktop_app_words_every_code() {
        let table = concat!(env!("CARGO_MANIFEST_DIR"), "/../../apps/desktop/src/lib/errors.ts");
        let table = std::fs::read_to_string(table).unwrap();
        let worded = |key: &str| table.contains(&format!("\"{key}\":")) || table.contains(&format!("  {key}:"));
        for e in samples() {
            assert!(worded(e.code()), "no wording for {}", e.code());
        }
        for reason in REASONS {
            let name = serde_json::to_value(reason).unwrap();
            assert!(worded(name.as_str().unwrap()), "no wording for block reason {name}");
        }
    }
}
