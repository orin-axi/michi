use crate::hints::Hint;
use crate::kv::{KvItem, KvValue};

/// Health classification for an individual status item.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum Health {
    /// Operating normally — no annotation shown.
    Ok,
    /// Degraded but still serving; carries a short reason.
    Degraded(String),
    /// Not serving; carries a short reason.
    Error(String),
}

/// A single named component with a value and optional health signal.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct StatusItem {
    /// Component key, e.g. `"index"`, `"cache"`.
    pub key: String,
    /// The component's actual value.
    pub value: KvValue,
    /// Optional health signal.
    pub health: Option<Health>,
}

/// Content-first orientation response (AXI **P8**).
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct StatusResponse {
    /// The tool's name, shown as the first line.
    pub tool_name: String,
    /// One-line description, shown as the second line.
    pub description: String,
    /// Component statuses.
    pub items: Vec<StatusItem>,
    /// Optional contextual hints for the agent.
    pub hints: Vec<Hint>,
}

impl StatusResponse {
    /// Create a status response.
    pub fn new(tool_name: impl Into<String>, description: impl Into<String>, items: Vec<StatusItem>) -> Self {
        Self { tool_name: tool_name.into(), description: description.into(), items, hints: Vec::new() }
    }

    /// Attach contextual hints.
    #[must_use]
    pub fn with_hints(mut self, hints: Vec<Hint>) -> Self {
        self.hints = hints;
        self
    }

    /// Render this status response as an agent-readable string.
    #[must_use]
    pub fn render(&self) -> String {
        let mut kv_items = Vec::with_capacity(self.items.len() + 2);
        kv_items.push(KvItem { key: "tool".to_string(), value: KvValue::Text(self.tool_name.clone()) });
        kv_items.push(KvItem { key: "description".to_string(), value: KvValue::Text(self.description.clone()) });
        for item in &self.items {
            let annotated = match &item.health {
                None | Some(Health::Ok) => item.value.clone(),
                Some(Health::Degraded(reason)) => annotate(&item.value, "DEGRADED", reason),
                Some(Health::Error(reason)) => annotate(&item.value, "ERROR", reason),
            };
            kv_items.push(KvItem { key: item.key.clone(), value: annotated });
        }
        crate::kv::render_kv(&kv_items, None, &self.hints)
    }
}

fn annotate(value: &KvValue, label: &str, reason: &str) -> KvValue {
    let mut display = String::new();
    crate::kv::push_kv_value(&mut display, value);
    KvValue::Text(format!("{display}  [{label}: {reason}]"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_and_description_lead_the_output() {
        let resp = StatusResponse::new("my-search-tool", "Semantic code search and symbol analysis", vec![]);
        let out = resp.render();
        assert!(
            out.starts_with("tool:        my-search-tool\ndescription: Semantic code search and symbol analysis\n"),
            "got: {out}"
        );
    }
}
