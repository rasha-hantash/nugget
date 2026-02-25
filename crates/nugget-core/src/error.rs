// ── Error Types ──

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NuggetError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parsing error: {0}")]
    Yaml(#[from] serde_yaml_ng::Error),

    #[error("missing frontmatter in {path}")]
    MissingFrontmatter { path: String },

    #[error("invalid frontmatter in {path}: {reason}")]
    InvalidFrontmatter { path: String, reason: String },
}

pub type Result<T> = std::result::Result<T, NuggetError>;
