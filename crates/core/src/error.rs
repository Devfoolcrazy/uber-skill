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
    #[error("not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io { path: path.into(), source }
    }
}

pub type Result<T> = std::result::Result<T, Error>;
