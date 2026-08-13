use crate::kv::KvValue;

/// A structured recovery hint for an agent encountering an error.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

/// Append a `recovery[N]:` block to an existing string in-place.
pub fn append_recovery(out: &mut String, hints: &[RecoveryHint]) {
    if hints.is_empty() {
        return;
    }
    out.push_str("recovery[");
    out.push_str(&hints.len().to_string());
    out.push_str("]:\n");
    append_recovery_lines(out, hints);
}

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
    fn empty_hints_returns_empty_string() {
        assert_eq!(render_recovery(&[]), "");
    }

    #[test]
    fn hint_with_no_params_renders_tool_name_only() {
        let hints = [RecoveryHint::new("retry_upload")];
        let out = render_recovery(&hints);
        assert_eq!(out, "recovery[1]:\n  retry_upload\n");
    }

    #[test]
    fn hint_with_reason_but_no_params() {
        let hints = [RecoveryHint::new("reload_cache").reason("cache entry expired")];
        let out = render_recovery(&hints);
        assert!(out.contains("reload_cache — cache entry expired"), "got: {out}");
        assert!(!out.contains("suggestedParams"), "got: {out}");
    }

    #[test]
    fn multiple_hints_all_appear() {
        let hints = [RecoveryHint::new("step_a"), RecoveryHint::new("step_b")];
        let out = render_recovery(&hints);
        assert_eq!(out, "recovery[2]:\n  step_a\n  step_b\n");
    }

    #[test]
    fn append_recovery_on_empty_slice_does_nothing() {
        let mut buf = "prefix\n".to_string();
        append_recovery(&mut buf, &[]);
        assert_eq!(buf, "prefix\n");
    }

    #[test]
    fn ac012_builder_methods_produce_expected_fields() {
        let hint = RecoveryHint::new("tool_name").param("a", KvValue::Int(1)).param("b", KvValue::Int(2)).reason("why");
        assert_eq!(hint.tool, "tool_name");
        assert_eq!(hint.params, vec![("a".to_string(), KvValue::Int(1)), ("b".to_string(), KvValue::Int(2))]);
        assert_eq!(hint.reason, Some("why".to_string()));

        let bare = RecoveryHint::new("tool_name");
        assert_eq!(bare.tool, "tool_name");
        assert!(bare.params.is_empty());
        assert_eq!(bare.reason, None);
    }

    #[test]
    fn ac015_multiple_params_are_comma_separated_in_insertion_order() {
        let hints = [RecoveryHint::new("assign_user")
            .param("user", KvValue::Text("alice".to_string()))
            .param("team", KvValue::Text("backend".to_string()))];
        let out = render_recovery(&hints);
        assert!(out.contains("assign_user: suggestedParams: { user: alice, team: backend }"), "got: {out}");
    }

    #[test]
    fn ac017_params_and_reason_appear_together_params_first() {
        let hints = [RecoveryHint::new("retry").param("attempt", KvValue::Int(2)).reason("transient network error")];
        let out = render_recovery(&hints);
        assert_eq!(out, "recovery[1]:\n  retry: suggestedParams: { attempt: 2 } — transient network error\n");
    }

    #[test]
    fn ac018a_append_recovery_appends_render_recovery_output_in_place() {
        let hints = [RecoveryHint::new("step_a"), RecoveryHint::new("step_b")];
        let mut out = "prefix\n".to_string();
        append_recovery(&mut out, &hints);
        assert_eq!(out, format!("prefix\n{}", render_recovery(&hints)));
    }

    #[test]
    fn ac018b_newline_in_tool_or_reason_passes_through_unstripped() {
        let hints = [RecoveryHint::new("a\nb").reason("c\nd")];
        let out = render_recovery(&hints);
        assert_eq!(out, "recovery[1]:\n  a\nb — c\nd\n");
        assert_eq!(out.lines().count(), 4, "two embedded newlines add two extra lines beyond the fixed count");
    }

    #[test]
    fn carriage_return_in_tool_or_reason_passes_through_but_does_not_change_line_count() {
        let hints = [RecoveryHint::new("a\rb").reason("c\rd")];
        let out = render_recovery(&hints);
        assert_eq!(out, "recovery[1]:\n  a\rb — c\rd\n");
        assert_eq!(out.lines().count(), 2);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn ac037b_serializes_with_null_for_unset_reason_and_no_rename() {
        assert_eq!(
            serde_json::to_string(&RecoveryHint::new("t")).unwrap(),
            "{\"tool\":\"t\",\"params\":[],\"reason\":null}"
        );
    }
}
