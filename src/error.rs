#[cfg(feature = "pipeline")]
use std::time::Duration;

/// Classification of a `michi::Error` for routing and display decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorClass {
    /// The caller provided invalid input. Do not retry.
    User,
    /// An internal or infrastructure failure. May be retryable.
    Internal,
    /// A transient failure expected to resolve without intervention.
    Transient,
}

/// Wraps a value so its inner content is omitted from `Debug` and `Display`.
///
/// Use this to prevent secrets (tokens, keys, passwords) from appearing in
/// logs or error messages.
#[derive(Clone)]
pub struct Sensitive<T>(pub T);

impl<T> std::fmt::Debug for Sensitive<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

impl<T> std::fmt::Display for Sensitive<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("<redacted>")
    }
}

/// The unified error type for the michi crate.
///
/// Carries both agent-renderable information (via [`Error::render`]) and
/// machine-readable classification (via [`Error::class`]).
///
/// Execution-layer variants (`Http`, `Timeout`, etc.) are only present when
/// the `pipeline` feature is enabled.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ── Domain errors (always compiled) ───────────────────────────────────
    /// The caller provided invalid or malformed input.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// A required resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    // ── Execution errors (pipeline feature — Plan 2) ───────────────────────
    /// An HTTP request returned a non-success status code.
    #[cfg(feature = "pipeline")]
    #[error("HTTP {status}: {message}")]
    Http {
        /// HTTP status code.
        status: u16,
        /// Error message from the response body.
        message: String,
        /// Whether this error is safe to retry.
        retryable: bool,
        /// Parsed `Retry-After` delay, if present.
        retry_after: Option<Duration>,
    },

    /// An operation exceeded its allotted time budget.
    #[cfg(feature = "pipeline")]
    #[error("Timeout after {elapsed:?}")]
    Timeout {
        /// How long the operation ran before timing out.
        elapsed: Duration,
    },

    /// A pipeline step failed during execution.
    #[cfg(feature = "pipeline")]
    #[error("Step {id} failed")]
    StepFailed {
        /// Step identifier.
        id: String,
        /// Underlying cause.
        #[source]
        source: Box<Self>,
    },

    /// A circuit breaker is open and rejecting calls.
    #[cfg(feature = "pipeline")]
    #[error("Circuit {name} is open, retry after {retry_after:?}")]
    CircuitOpen {
        /// Circuit breaker name.
        name: String,
        /// When the circuit is expected to allow retries.
        retry_after: Duration,
    },

    /// A fuzzy-match query produced no candidates.
    #[cfg(feature = "pipeline")]
    #[error("No match for query: {query}")]
    NoMatch {
        /// The query that produced no matches.
        query: String,
    },

    /// A fuzzy-match query produced multiple equally-likely candidates.
    #[cfg(feature = "pipeline")]
    #[error("Ambiguous match for '{query}': {count} candidates")]
    AmbiguousMatch {
        /// The query that produced multiple matches.
        query: String,
        /// Number of candidates found.
        count: usize,
    },

    /// A pipeline's step dependencies form a cycle.
    #[cfg(feature = "pipeline")]
    #[error("Cyclic dependency detected: {cycle:?}")]
    CyclicDependency {
        /// The cycle path, as a list of step IDs.
        cycle: Vec<String>,
    },

    /// The operation was cancelled before completion.
    #[cfg(feature = "pipeline")]
    #[error("Operation was cancelled")]
    Cancelled,

    /// The cache backend encountered an error.
    #[cfg(feature = "pipeline")]
    #[error("Cache error: {0}")]
    Cache(String),

    /// An underlying I/O operation failed.
    #[cfg(feature = "pipeline")]
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl Error {
    /// Render this error as an agent-readable plain-text string.
    ///
    /// The output is suitable for writing to stdout before exiting with
    /// [`Error::exit_code`].
    #[must_use]
    pub fn render(&self) -> String {
        format!("error: {self}")
    }

    /// The process exit code to use when this error is fatal.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        1
    }

    /// Classify this error for routing decisions (retry, alert, display).
    #[must_use]
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::InvalidInput(_) | Self::NotFound(_) => ErrorClass::User,
            #[cfg(feature = "pipeline")]
            Self::Http { retryable: true, .. } | Self::Timeout { .. } | Self::Cancelled => ErrorClass::Transient,
            #[cfg(feature = "pipeline")]
            Self::StepFailed { source, .. } => source.class(),
            #[cfg(feature = "pipeline")]
            Self::Http { retryable: false, .. }
            | Self::CircuitOpen { .. }
            | Self::NoMatch { .. }
            | Self::AmbiguousMatch { .. }
            | Self::CyclicDependency { .. }
            | Self::Cache(_)
            | Self::Io(_) => ErrorClass::Internal,
        }
    }

    /// Whether this error is safe to retry automatically.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        matches!(self.class(), ErrorClass::Transient)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_input_renders() {
        let e = Error::InvalidInput("field 'name' is required".into());
        assert_eq!(e.render(), "error: Invalid input: field 'name' is required");
    }

    #[test]
    fn not_found_renders() {
        let e = Error::NotFound("issue #99".into());
        assert_eq!(e.render(), "error: Not found: issue #99");
    }

    #[test]
    fn invalid_input_is_user_class() {
        let e = Error::InvalidInput("bad".into());
        assert_eq!(e.class(), ErrorClass::User);
        assert!(!e.is_retryable());
    }

    #[test]
    fn not_found_is_user_class() {
        let e = Error::NotFound("thing".into());
        assert_eq!(e.class(), ErrorClass::User);
    }

    #[test]
    fn exit_code_is_one() {
        let e = Error::NotFound("x".into());
        assert_eq!(e.exit_code(), 1);
    }

    #[test]
    fn sensitive_redacts_debug() {
        let s = Sensitive("secret-token");
        assert_eq!(format!("{s:?}"), "<redacted>");
    }

    #[test]
    fn sensitive_redacts_display() {
        let s = Sensitive("secret-token");
        assert_eq!(format!("{s}"), "<redacted>");
    }

    #[cfg(feature = "pipeline")]
    #[test]
    fn step_failed_delegates_class_to_transient_source() {
        let e = Error::StepFailed {
            id: "fetch".into(),
            source: Box::new(Error::Http { status: 503, message: "busy".into(), retryable: true, retry_after: None }),
        };
        assert_eq!(e.class(), ErrorClass::Transient);
        assert!(e.is_retryable());
    }

    #[cfg(feature = "pipeline")]
    #[test]
    fn step_failed_delegates_class_to_user_source() {
        let e = Error::StepFailed { id: "validate".into(), source: Box::new(Error::InvalidInput("bad field".into())) };
        assert_eq!(e.class(), ErrorClass::User);
        assert!(!e.is_retryable());
    }
}
