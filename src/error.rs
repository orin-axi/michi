use std::time::Duration;

use crate::hints::Hint;
use crate::recovery::RecoveryHint;

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

/// The specific kind of domain error, independent of the pipeline-execution
/// error variants. Each code has a default retryability and renders in
/// snake_case via [`ErrorCode::label`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    /// Bad parameters. Non-retryable — the agent must change something first.
    InvalidInput,
    /// Resource absent. Non-retryable.
    NotFound,
    /// Auth failure. Non-retryable.
    Unauthorized,
    /// Permission denied. Non-retryable.
    Forbidden,
    /// Resource state mismatch. Non-retryable.
    Conflict,
    /// Rate limited (HTTP 429). Retryable — check `retry_after`.
    RateLimited,
    /// Service unavailable (HTTP 503). Retryable.
    Unavailable,
    /// Request timed out. Retryable.
    Timeout,
    /// Downstream/external failure. Retryable.
    ExternalFailure,
}

impl ErrorCode {
    /// The snake_case label used in rendered output.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::InvalidInput => "invalid_input",
            Self::NotFound => "not_found",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::Conflict => "conflict",
            Self::RateLimited => "rate_limited",
            Self::Unavailable => "unavailable",
            Self::Timeout => "timeout",
            Self::ExternalFailure => "external_failure",
        }
    }

    /// Whether this code is conventionally retryable, absent an explicit
    /// override via [`DomainError::retryable`].
    #[must_use]
    pub fn is_retryable_by_default(&self) -> bool {
        matches!(self, Self::RateLimited | Self::Unavailable | Self::Timeout | Self::ExternalFailure)
    }
}

/// A domain-level error: a classified code, message, and everything needed
/// to render an agent-actionable response — hints, an optional structured
/// recovery hint, and retry metadata. The HTTP-status-to-`ErrorCode` mapping
/// is deliberately not provided here — callers interpret their own failures
/// into an `ErrorCode`, keeping this module free of HTTP knowledge.
#[derive(Debug, Clone)]
pub struct DomainError {
    /// The error classification.
    pub code: ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Contextual hints, rendered as a trailing `help[N]:` block.
    pub hints: Vec<Hint>,
    /// Optional structured recovery hint.
    pub recovery: Option<RecoveryHint>,
    /// Whether this error is safe to retry. Defaults to `code.is_retryable_by_default()`.
    pub retryable: bool,
    /// Parsed `Retry-After` delay, if known.
    pub retry_after: Option<Duration>,
}

impl DomainError {
    /// Create a domain error. `retryable` defaults from `code`.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            retryable: code.is_retryable_by_default(),
            code,
            message: message.into(),
            hints: Vec::new(),
            recovery: None,
            retry_after: None,
        }
    }

    /// Append a contextual hint.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hints.push(Hint::new(hint));
        self
    }

    /// Attach a structured recovery hint.
    #[must_use]
    pub fn recovery(mut self, r: RecoveryHint) -> Self {
        self.recovery = Some(r);
        self
    }

    /// Override the default retryability for this specific error instance.
    #[must_use]
    pub fn retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    /// Attach a `Retry-After` delay.
    #[must_use]
    pub fn retry_after(mut self, d: Duration) -> Self {
        self.retry_after = Some(d);
        self
    }

    /// The process exit code to use when this error is fatal. Always `1`.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        1
    }

    /// Render to an agent-readable KV block with exit code, hints, and recovery.
    ///
    /// Format:
    /// ```text
    /// error: not_found
    /// message: Issue #9999 does not exist in this repository
    /// exit_code: 1
    /// help[1]:
    ///   Call list_issues to see available numbers
    /// ```
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(64 + self.message.len() + self.hints.len() * 50);
        out.push_str("error: ");
        out.push_str(self.code.label());
        out.push_str("\nmessage: ");
        out.push_str(&self.message);
        out.push_str("\nexit_code: 1\n");
        crate::hints::append_hints(&mut out, &self.hints);
        if let Some(r) = &self.recovery {
            crate::recovery::append_recovery(&mut out, std::slice::from_ref(r));
        }
        out
    }
}

/// The unified error type for the michi crate.
///
/// Carries both agent-renderable information (via [`Error::render`]) and
/// machine-readable classification (via [`Error::class`]).
///
/// Execution-layer variants (`Http`, `Timeout`, etc.) return when the
/// `pipeline` crate (Plan 2) lands — see `docs/spec/06-decisions.md`'s
/// crate-boundary entry.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    // ── Domain errors (always compiled) ───────────────────────────────────
    /// The caller provided invalid or malformed input.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// A required resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// A classified domain error with hints, recovery, and retry metadata.
    #[error("{}: {}", .0.code.label(), .0.message)]
    Domain(DomainError),
}

impl Error {
    /// Render this error as an agent-readable plain-text string.
    ///
    /// The output is suitable for writing to stdout before exiting with
    /// [`Error::exit_code`].
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Domain(d) => d.render(),
            other => format!("error: {other}"),
        }
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
            Self::Domain(d) if d.retryable => ErrorClass::Transient,
            Self::InvalidInput(_) | Self::NotFound(_) | Self::Domain(_) => ErrorClass::User,
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

    #[test]
    fn error_code_labels_are_snake_case() {
        assert_eq!(ErrorCode::InvalidInput.label(), "invalid_input");
        assert_eq!(ErrorCode::NotFound.label(), "not_found");
        assert_eq!(ErrorCode::Unauthorized.label(), "unauthorized");
        assert_eq!(ErrorCode::Forbidden.label(), "forbidden");
        assert_eq!(ErrorCode::Conflict.label(), "conflict");
        assert_eq!(ErrorCode::RateLimited.label(), "rate_limited");
        assert_eq!(ErrorCode::Unavailable.label(), "unavailable");
        assert_eq!(ErrorCode::Timeout.label(), "timeout");
        assert_eq!(ErrorCode::ExternalFailure.label(), "external_failure");
    }

    #[test]
    fn non_retryable_codes() {
        for code in [
            ErrorCode::InvalidInput,
            ErrorCode::NotFound,
            ErrorCode::Unauthorized,
            ErrorCode::Forbidden,
            ErrorCode::Conflict,
        ] {
            assert!(!code.is_retryable_by_default(), "{code:?} should not be retryable by default");
        }
    }

    #[test]
    fn retryable_codes() {
        for code in [ErrorCode::RateLimited, ErrorCode::Unavailable, ErrorCode::Timeout, ErrorCode::ExternalFailure] {
            assert!(code.is_retryable_by_default(), "{code:?} should be retryable by default");
        }
    }

    #[test]
    fn domain_error_renders_kv_block_with_hints() {
        let e = DomainError::new(ErrorCode::NotFound, "Issue #9999 does not exist in this repository")
            .hint("Call list_issues to see available numbers");
        let out = e.render();
        assert_eq!(
            out,
            "error: not_found\nmessage: Issue #9999 does not exist in this repository\nexit_code: 1\nhelp[1]:\n  Call list_issues to see available numbers\n"
        );
    }

    #[test]
    fn domain_error_exit_code_is_always_one() {
        let e = DomainError::new(ErrorCode::RateLimited, "slow down");
        assert_eq!(e.exit_code(), 1);
    }

    #[test]
    fn domain_error_retryable_defaults_from_code_but_is_overridable() {
        let default_retryable = DomainError::new(ErrorCode::RateLimited, "x");
        assert!(default_retryable.retryable);
        let overridden = DomainError::new(ErrorCode::NotFound, "x").retryable(true);
        assert!(overridden.retryable, "explicit .retryable() call overrides the code's default");
    }

    #[test]
    fn domain_error_carries_recovery() {
        let e = DomainError::new(ErrorCode::Conflict, "already exists").recovery(RecoveryHint::new("get_issue"));
        assert_eq!(e.recovery.as_ref().unwrap().tool, "get_issue");
    }

    #[test]
    fn domain_error_render_includes_recovery_block() {
        let e = DomainError::new(ErrorCode::Conflict, "already exists").recovery(RecoveryHint::new("get_issue"));
        let out = e.render();
        assert_eq!(out, "error: conflict\nmessage: already exists\nexit_code: 1\nrecovery[1]:\n  get_issue\n");
    }

    #[test]
    fn domain_error_render_with_no_hints_has_no_dangling_help_block() {
        let e = DomainError::new(ErrorCode::Timeout, "took too long");
        assert_eq!(e.render(), "error: timeout\nmessage: took too long\nexit_code: 1\n");
    }

    #[test]
    fn error_domain_variant_wraps_domain_error() {
        let e = Error::Domain(DomainError::new(ErrorCode::NotFound, "gone"));
        assert_eq!(e.class(), ErrorClass::User);
        assert!(!e.is_retryable());
        assert_eq!(e.exit_code(), 1);
        assert!(e.render().starts_with("error: not_found\n"));
    }
}
