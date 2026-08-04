use std::time::Duration;

use crate::hints::Hint;
use crate::recovery::RecoveryHint;

/// Classification of a `michi::Error` for routing and display decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum ErrorClass {
    /// The caller provided invalid input. Do not retry.
    User,
    /// A downstream or infrastructure failure that is not the caller's direct
    /// error and is not expected to self-resolve without intervention. Note:
    /// `RateLimited` falls here when `retryable: false` — the caller may have
    /// contributed to the rate-limit condition, but at classification time the
    /// error is not self-resolving. michi does not recommend automatic retry
    /// for `Internal` errors.
    Internal,
    /// A transient failure expected to resolve without intervention.
    Transient,
}

/// Wraps a value so its inner content is omitted from `Debug` and `Display`.
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

/// The specific kind of domain error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum ErrorCode {
    /// Bad parameters.
    InvalidInput,
    /// Resource absent.
    NotFound,
    /// Auth failure.
    Unauthorized,
    /// Permission denied.
    Forbidden,
    /// Resource state mismatch.
    Conflict,
    /// Rate limited (HTTP 429).
    RateLimited,
    /// Service unavailable (HTTP 503).
    Unavailable,
    /// Request timed out.
    Timeout,
    /// Downstream/external failure.
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

    /// Whether this code is conventionally retryable.
    #[must_use]
    pub fn is_retryable_by_default(&self) -> bool {
        matches!(self, Self::RateLimited | Self::Unavailable | Self::Timeout | Self::ExternalFailure)
    }

    /// The default [`ErrorClass`] for this code, independent of `DomainError.retryable`.
    ///
    /// Infrastructure codes (`RateLimited`, `Unavailable`, `Timeout`,
    /// `ExternalFailure`) return `Internal`; caller-fault codes return `User`.
    /// Used by [`Error::class()`].
    #[must_use]
    pub fn default_class(&self) -> ErrorClass {
        match self {
            Self::InvalidInput | Self::NotFound | Self::Unauthorized | Self::Forbidden | Self::Conflict => {
                ErrorClass::User
            }
            Self::RateLimited | Self::Unavailable | Self::Timeout | Self::ExternalFailure => ErrorClass::Internal,
        }
    }
}

/// A domain-level error.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct DomainError {
    /// The error classification.
    pub code: ErrorCode,
    /// Human-readable message.
    pub message: String,
    /// Contextual hints, rendered as a trailing `help[N]:` block.
    pub hints: Vec<Hint>,
    /// Optional structured recovery hint.
    pub recovery: Option<RecoveryHint>,
    /// Whether this error is safe to retry.
    pub retryable: bool,
    /// Parsed `Retry-After` delay, if known.
    pub retry_after: Option<Duration>,
}

impl DomainError {
    /// Create a domain error.
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

    /// Override the default retryability for this error instance.
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

    /// Render formatted for GitHub Actions workflow annotations (`::error title=code::message`).
    #[must_use]
    pub fn render_github_annotation(&self) -> String {
        format!("::error title={}::{}", self.code.label(), self.message.replace('\n', "%0A"))
    }

    /// Render as a JSON object matching `AgentResponse::render_json()` shape.
    ///
    /// Keys: `isError`, `error`, `message`, `retryable`, `hints`, `recovery`.
    /// Client code reading `structured_content` should not need two schemas.
    #[must_use]
    pub fn render_json(&self) -> String {
        let mut out = String::with_capacity(128 + self.message.len());
        out.push_str("{\"isError\":true,\"error\":");
        crate::kv::json_escape_str(&mut out, self.code.label());
        out.push_str(",\"message\":");
        crate::kv::json_escape_str(&mut out, &self.message);
        out.push_str(",\"retryable\":");
        out.push_str(if self.retryable { "true" } else { "false" });
        out.push_str(",\"hints\":[");
        for (i, h) in self.hints.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            crate::kv::json_escape_str(&mut out, h.as_str());
        }
        out.push_str("],\"recovery\":");
        match &self.recovery {
            None => out.push_str("null"),
            Some(r) => {
                out.push_str("{\"tool\":");
                crate::kv::json_escape_str(&mut out, &r.tool);
                out.push_str(",\"params\":{");
                for (j, (k, v)) in r.params.iter().enumerate() {
                    if j > 0 {
                        out.push(',');
                    }
                    crate::kv::json_escape_str(&mut out, k);
                    out.push(':');
                    crate::kv::kv_value_to_json(&mut out, v);
                }
                out.push('}');
                if let Some(reason) = &r.reason {
                    out.push_str(",\"reason\":");
                    crate::kv::json_escape_str(&mut out, reason);
                }
                out.push('}');
            }
        }
        out.push('}');
        out
    }

    /// Build an MCP [`CallToolResult`] representing this error.
    ///
    /// Sets `is_error: true`. Text content uses [`DomainError::render()`];
    /// `structured_content` uses [`DomainError::render_json()`].
    ///
    /// **Canonical path:** call this directly when you hold a `DomainError`.
    /// Do not wrap in `AgentResponse::as_error()` — that loses typed fields.
    #[must_use]
    pub fn to_call_tool_result(&self) -> crate::mcp::CallToolResult {
        crate::mcp::CallToolResult {
            content: vec![crate::mcp::ContentBlock {
                text: self.render(),
                audience: vec![crate::audience::Audience::Assistant],
            }],
            is_error: true,
            structured_content: self.render_json(),
        }
    }
}

impl std::fmt::Display for DomainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code.label(), self.message)
    }
}

impl std::error::Error for DomainError {}

#[cfg(feature = "miette")]
impl miette::Diagnostic for DomainError {
    fn code<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        Some(Box::new(self.code.label()))
    }

    fn help<'a>(&'a self) -> Option<Box<dyn std::fmt::Display + 'a>> {
        if self.hints.is_empty() {
            None
        } else {
            Some(Box::new(self.hints.iter().map(|h| h.as_str()).collect::<Vec<_>>().join("\n")))
        }
    }
}

/// The unified error type for the michi crate.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// Invalid or malformed input.
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// A required resource was not found.
    #[error("Not found: {0}")]
    NotFound(String),

    /// A classified domain error with hints, recovery, and retry metadata.
    #[error("{}: {}", .0.code.label(), .0.message)]
    Domain(#[from] DomainError),
}

impl Error {
    /// Render this error as an agent-readable plain-text string.
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

    /// Classify this error for routing decisions.
    #[must_use]
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::Domain(d) if d.retryable => ErrorClass::Transient,
            Self::Domain(d) => d.code.default_class(),
            Self::InvalidInput(_) | Self::NotFound(_) => ErrorClass::User,
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
    fn domain_error_render_json_basic() {
        let err = DomainError::new(ErrorCode::NotFound, "Item 42 not found");
        let json = err.render_json();
        assert!(json.contains("\"isError\":true"), "got: {json}");
        assert!(json.contains("\"error\":"), "got: {json}");
        assert!(json.contains("\"message\":\"Item 42 not found\""), "got: {json}");
        assert!(json.contains("\"retryable\":false"), "got: {json}");
        assert!(json.contains("\"hints\":["), "got: {json}");
    }

    #[test]
    fn domain_error_render_json_includes_hints() {
        let err = DomainError::new(ErrorCode::NotFound, "not found").hint("Try searching with get_items");
        let json = err.render_json();
        assert!(json.contains("\"hints\""), "got: {json}");
        assert!(json.contains("Try searching"), "got: {json}");
    }

    #[test]
    fn domain_error_to_call_tool_result_is_error() {
        let err = DomainError::new(ErrorCode::Unavailable, "down");
        let result = err.to_call_tool_result();
        assert!(result.is_error);
        assert!(!result.content.is_empty());
    }

    #[test]
    fn domain_error_json_escapes_message() {
        let err = DomainError::new(ErrorCode::InvalidInput, r#"field "name" invalid"#);
        let json = err.render_json();
        assert!(json.contains("\\\"name\\\""), "quotes in message must be JSON-escaped, got: {json}");
    }

    #[test]
    fn invalid_input_renders() {
        let e = Error::InvalidInput("field 'name' is required".into());
        assert_eq!(e.render(), "error: Invalid input: field 'name' is required");
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
    fn domain_error_renders_github_annotation() {
        let e = DomainError::new(ErrorCode::InvalidInput, "field 'name' is required");
        assert_eq!(e.render_github_annotation(), "::error title=invalid_input::field 'name' is required");
    }

    #[test]
    fn unavailable_with_retryable_false_is_internal_not_user() {
        let err = Error::Domain(DomainError::new(ErrorCode::Unavailable, "down").retryable(false));
        assert_eq!(err.class(), ErrorClass::Internal, "infra error with retryable=false must be Internal, not User");
    }

    #[test]
    fn invalid_input_is_user() {
        let err = Error::Domain(DomainError::new(ErrorCode::InvalidInput, "bad input"));
        assert_eq!(err.class(), ErrorClass::User);
    }

    #[test]
    fn rate_limited_retryable_true_is_transient() {
        // Default: RateLimited is retryable by default
        let err = Error::Domain(DomainError::new(ErrorCode::RateLimited, "429"));
        assert_eq!(err.class(), ErrorClass::Transient);
    }

    #[test]
    fn default_class_for_all_codes() {
        use ErrorCode::*;
        assert_eq!(InvalidInput.default_class(), ErrorClass::User);
        assert_eq!(NotFound.default_class(), ErrorClass::User);
        assert_eq!(Unauthorized.default_class(), ErrorClass::User);
        assert_eq!(Forbidden.default_class(), ErrorClass::User);
        assert_eq!(Conflict.default_class(), ErrorClass::User);
        assert_eq!(RateLimited.default_class(), ErrorClass::Internal);
        assert_eq!(Unavailable.default_class(), ErrorClass::Internal);
        assert_eq!(Timeout.default_class(), ErrorClass::Internal);
        assert_eq!(ExternalFailure.default_class(), ErrorClass::Internal);
    }

    #[test]
    fn internal_class_is_not_retryable() {
        // Internal is not Transient, so is_retryable() must return false
        let err = Error::Domain(DomainError::new(ErrorCode::Unavailable, "down").retryable(false));
        assert!(!err.is_retryable(), "Internal-classified error must not be retryable");
    }
}
