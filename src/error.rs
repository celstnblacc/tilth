use std::path::PathBuf;

/// Every error tilth can produce. Displayed as user-facing messages with suggestions.
#[derive(Debug)]
pub enum TilthError {
    NotFound {
        path: PathBuf,
        suggestion: Option<String>,
    },
    PermissionDenied {
        path: PathBuf,
    },
    InvalidQuery {
        query: String,
        reason: String,
    },
    IoError {
        path: PathBuf,
        source: std::io::Error,
    },
    ParseError {
        path: PathBuf,
        reason: String,
    },
    PathTraversal {
        path: PathBuf,
        scope: PathBuf,
    },
}

impl std::fmt::Display for TilthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound { path, suggestion } => {
                write!(f, "not found: {}", path.display())?;
                if let Some(s) = suggestion {
                    write!(f, " — did you mean: {s}")?;
                }
                Ok(())
            }
            Self::PermissionDenied { path } => {
                write!(f, "{} [permission denied]", path.display())
            }
            Self::InvalidQuery { query, reason } => {
                write!(f, "invalid query \"{query}\": {reason}")
            }
            Self::IoError { path, source } => {
                write!(f, "{}: {source}", path.display())
            }
            Self::ParseError { path, reason } => {
                write!(f, "parse error in {}: {reason}", path.display())
            }
            Self::PathTraversal { path, scope } => write!(
                f,
                "path traversal blocked: {} is outside scope {}",
                path.display(),
                scope.display()
            ),
        }
    }
}

impl std::error::Error for TilthError {}

impl TilthError {
    /// Exit code matching the spec.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NotFound { .. } | Self::IoError { .. } => 2,
            Self::InvalidQuery { .. } | Self::ParseError { .. } => 3,
            Self::PermissionDenied { .. } | Self::PathTraversal { .. } => 4,
        }
    }
}
