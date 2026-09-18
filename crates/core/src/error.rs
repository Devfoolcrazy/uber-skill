use std::path::PathBuf;

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
    /// Git could not be started, or another Git operation holds the lock.
    #[error("{0}")]
    GitUnavailable(String),
    /// Git ran and failed; the message carries its stderr.
    #[error("{0}")]
    GitCommand(String),
    /// The repository or library changed since the reviewed preview or check.
    #[error("{0}")]
    GitStale(String),
    /// The repository state must be resolved in an external Git tool first.
    #[error("{0}")]
    GitBlocked(String),
    /// A value supplied by the caller was rejected before anything ran.
    #[error("{0}")]
    InvalidInput(String),
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
}

pub type Result<T> = std::result::Result<T, Error>;
