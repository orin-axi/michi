/// A structured recovery hint for an agent encountering an error.
///
/// Recovery hints give the agent concrete next steps when an operation fails,
/// rather than a bare error message.
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryHint {
    /// Short action label, e.g. `"retry"`, `"check_auth"`.
    pub action: String,
    /// Human-readable description of what this recovery achieves.
    pub description: String,
    /// An optional example invocation the agent can copy verbatim.
    pub example: Option<String>,
}

impl RecoveryHint {
    /// Create a recovery hint with no example.
    pub fn new(action: impl Into<String>, description: impl Into<String>) -> Self {
        Self { action: action.into(), description: description.into(), example: None }
    }

    /// Create a recovery hint with an example command or call.
    pub fn with_example(action: impl Into<String>, description: impl Into<String>, example: impl Into<String>) -> Self {
        Self { action: action.into(), description: description.into(), example: Some(example.into()) }
    }
}

/// Render a list of recovery hints as an agent-readable block.
///
/// Format:
/// ```text
/// recovery[2]:
///   retry: call the endpoint again after 1s
///   check_auth: verify your API key is valid → get_token()
/// ```
///
/// Returns an empty string when `hints` is empty.
#[must_use]
pub fn render_recovery(hints: &[RecoveryHint]) -> String {
    if hints.is_empty() {
        return String::new();
    }
    let capacity = 16 + hints.len() * 60;
    let mut out = String::with_capacity(capacity);
    append_recovery(&mut out, hints);
    out
}

/// Append a `recovery[N]:` block to an existing string in-place, without
/// allocating an intermediate buffer.
///
/// No-op when `hints` is empty.
pub fn append_recovery(out: &mut String, hints: &[RecoveryHint]) {
    if hints.is_empty() {
        return;
    }
    out.push_str("recovery[");
    out.push_str(&hints.len().to_string());
    out.push_str("]:\n");
    for hint in hints {
        out.push_str("  ");
        out.push_str(&hint.action);
        out.push_str(": ");
        out.push_str(&hint.description);
        if let Some(example) = &hint.example {
            out.push_str(" → ");
            out.push_str(example);
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_hint_no_example() {
        let hints = [RecoveryHint::new("retry", "call the endpoint again after 1s")];
        assert_eq!(render_recovery(&hints), "recovery[1]:\n  retry: call the endpoint again after 1s\n");
    }

    #[test]
    fn hint_with_example() {
        let hints = [RecoveryHint::with_example("check_auth", "verify your API key", "get_token()")];
        let out = render_recovery(&hints);
        assert!(out.contains("check_auth: verify your API key → get_token()"));
    }

    #[test]
    fn empty_hints_returns_empty() {
        assert_eq!(render_recovery(&[]), "");
    }

    #[test]
    fn multiple_hints_renders_count() {
        let hints = [RecoveryHint::new("retry", "wait and retry"), RecoveryHint::new("escalate", "contact support")];
        let out = render_recovery(&hints);
        assert!(out.starts_with("recovery[2]:\n"));
        assert!(out.contains("  retry: wait and retry\n"));
        assert!(out.contains("  escalate: contact support\n"));
    }

    #[test]
    fn append_recovery_modifies_string() {
        let mut s = "body\n".to_string();
        append_recovery(&mut s, &[RecoveryHint::new("retry", "wait and retry")]);
        assert_eq!(s, "body\nrecovery[1]:\n  retry: wait and retry\n");
    }

    #[test]
    fn append_recovery_noop_when_empty() {
        let mut s = "base".to_string();
        append_recovery(&mut s, &[]);
        assert_eq!(s, "base");
    }
}
