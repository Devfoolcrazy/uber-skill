//! uber-skill-core: manage a library of agent skills (folders with a `SKILL.md`),
//! install them into projects, and track drift between library and project copies.

pub mod config;
pub mod error;
pub mod frontmatter;
pub mod fsutil;
pub mod git;
pub mod index;
pub mod install;
pub mod library;
pub mod lint;
pub mod model;
pub mod projects;
pub mod refine;
pub mod registry;
pub mod search;
pub mod suggest;
pub mod targets;

pub use config::Config;
pub use error::{BlockReason, Error, ErrorPayload, Result};
pub use install::{DriftState, FileDiff, InstalledSkill, LockEntry, LockFile};
pub use library::Library;
pub use lint::{Issue, Severity};
pub use model::{ItemKind, ScanResult, ScanWarning, Skill};
pub use registry::{Facet, Registry, RegistryView};
pub use search::Query;
pub use targets::Target;
