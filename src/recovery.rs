use crate::kv::KvValue;

/// A structured recovery hint for an agent encountering an error.
///
/// Names a tool to call and, optionally, the parameters to call it with —
/// machine-actionable, not just descriptive text. Rendered as part of a
/// `recovery[N]:` block (see [`render_recovery`]).
#[derive(Debug, Clone, PartialEq)]
pub struct RecoveryHint {
    /// The tool/operation name the agent should call to recover.
    pub tool: String,
    /// Ordered key-value parameters to call `tool` with.
    pub params: Vec<(String, KvValue)>,
    /// Optional human-readable reason this recovery path applies.
    pub reason: Option<String>,
}

impl RecoveryHint {
    /// Create a recovery hint naming just the tool to call.
    pub fn new(tool: impl Into<String>) -> Self {
        Self { tool: tool.into(), params: Vec::new(), reason: None }
    }

    /// Append a suggested parameter.
    #[must_use]
    pub fn param(mut self, key: impl Into<String>, value: KvValue) -> Self {
        self.params.push((key.into(), value));
        self
    }

    /// Attach a human-readable reason.
    #[must_use]
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }
}

/// Render a list of recovery hints as an agent-readable block.
///
/// Format:
/// ```text
/// recovery[2]:
///   assign_user: suggestedParams: { user: alice } — user 'ghost' not found
///   list_issues
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
    append_recovery_lines(out, hints);
}

/// Append the per-hint lines only (no `recovery[N]:` header).
///
/// Shared by [`append_recovery`] and other blocks that fold recovery hints
/// under a different header — e.g. `idempotency::PartialSuccess::render`,
/// which folds them into a trailing `help[N]:` block.
pub(crate) fn append_recovery_lines(out: &mut String, hints: &[RecoveryHint]) {
    for hint in hints {
        out.push_str("  ");
        out.push_str(&hint.tool);
        if !hint.params.is_empty() {
            out.push_str(": suggestedParams: { ");
            for (i, (k, v)) in hint.params.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(k);
                out.push_str(": ");
                crate::kv::push_kv_value(out, v);
            }
            out.push_str(" }");
        }
        if let Some(reason) = &hint.reason {
            out.push_str(" — ");
            out.push_str(reason);
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hint_with_params_renders_suggested_params() {
        let hints = [RecoveryHint::new("assign_user").param("user", KvValue::Text("alice".to_string()))];
        let out = render_recovery(&hints);
        assert!(out.contains("assign_user: suggestedParams: { user: alice }"), "got: {out}");
    }

    #[test]
    fn hint_with_multiple_params_renders_all() {
        let hints = [RecoveryHint::new("create_item")
            .param("project", KvValue::Text("PROJ".to_string()))
            .param("type", KvValue::Text("Task".to_string()))];
        let out = render_recovery(&hints);
        assert!(out.contains("project: PROJ"), "got: {out}");
        assert!(out.contains("type: Task"), "got: {out}");
    }

    #[test]
    fn hint_with_reason_includes_it() {
        let hints = [RecoveryHint::new("retry_call").reason("rate limit hit")];
        let out = render_recovery(&hints);
        assert!(out.contains("retry_call"));
        assert!(out.contains("rate limit hit"));
    }

    #[test]
    fn hint_with_params_and_reason_renders_both_in_order() {
        let hints = [RecoveryHint::new("assign_user")
            .param("user", KvValue::Text("alice".to_string()))
            .reason("user 'ghost' not found")];
        let out = render_recovery(&hints);
        assert_eq!(out, "recovery[1]:\n  assign_user: suggestedParams: { user: alice } — user 'ghost' not found\n");
    }

    #[test]
    fn hint_with_no_params_no_reason_renders_bare_tool_name() {
        let hints = [RecoveryHint::new("list_issues")];
        let out = render_recovery(&hints);
        assert!(out.contains("recovery[1]:\n  list_issues\n"), "got: {out}");
    }

    #[test]
    fn empty_hints_returns_empty() {
        assert_eq!(render_recovery(&[]), "");
    }

    #[test]
    fn multiple_hints_renders_count() {
        let hints = [RecoveryHint::new("retry"), RecoveryHint::new("escalate")];
        let out = render_recovery(&hints);
        assert!(out.starts_with("recovery[2]:\n"));
    }

    #[test]
    fn append_recovery_modifies_string() {
        let mut s = "body\n".to_string();
        append_recovery(&mut s, &[RecoveryHint::new("retry")]);
        assert_eq!(s, "body\nrecovery[1]:\n  retry\n");
    }

    #[test]
    fn append_recovery_noop_when_empty() {
        let mut s = "base".to_string();
        append_recovery(&mut s, &[]);
        assert_eq!(s, "base");
    }

    #[test]
    fn int_and_bool_params_render_correctly() {
        let hints =
            [RecoveryHint::new("retry_after").param("seconds", KvValue::Int(30)).param("force", KvValue::Bool(true))];
        let out = render_recovery(&hints);
        assert!(out.contains("seconds: 30"), "got: {out}");
        assert!(out.contains("force: true"), "got: {out}");
    }

    #[test]
    fn float_param_respects_stored_decimal_precision() {
        let hints = [RecoveryHint::new("t").param("ratio", KvValue::Float(1.0 / 3.0, 2))];
        let out = render_recovery(&hints);
        assert!(out.contains("ratio: 0.33"), "got: {out}");
        assert!(!out.contains("0.333333"), "got: {out}");
    }

    #[test]
    fn float_param_precision_matches_kv_render_kv_for_same_value() {
        let hints = [RecoveryHint::new("t").param("ratio", KvValue::Float(1.0 / 3.0, 2))];
        let recovery_out = render_recovery(&hints);
        let mut kv_out = String::new();
        crate::kv::push_kv_value(&mut kv_out, &KvValue::Float(1.0 / 3.0, 2));
        assert!(
            recovery_out.contains(&format!("ratio: {kv_out}")),
            "same KvValue must render identically via both paths, got: {recovery_out}"
        );
    }
}
