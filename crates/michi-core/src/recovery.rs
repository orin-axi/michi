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
}
