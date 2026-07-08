use crate::hints::Hint;
use crate::kv::{KvItem, KvValue};

/// Health classification for an individual status item.
///
/// `Ok` renders with no annotation. `Degraded`/`Error` carry a reason and
/// render as a trailing `[DEGRADED: reason]`/`[ERROR: reason]` annotation
/// after the item's own value — health is a signal alongside the real data,
/// not a replacement for it.
#[derive(Debug, Clone, PartialEq, Eq)]
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
pub struct StatusItem {
    /// Component key, e.g. `"index"`, `"cache"`.
    pub key: String,
    /// The component's actual value — what a caller would want to read.
    pub value: KvValue,
    /// Optional health signal. `None` and `Some(Health::Ok)` both render with
    /// no bracket annotation; only `Degraded`/`Error` add one.
    pub health: Option<Health>,
}

/// Content-first orientation response (AXI **P8**): what a tool returns when
/// called with no arguments. Built on [`crate::kv::render_kv`].
#[derive(Debug, Clone, PartialEq)]
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
    use crate::hints::Hint;
    use crate::kv::KvValue;

    #[test]
    fn tool_and_description_lead_the_output() {
        let resp = StatusResponse::new("my-search-tool", "Semantic code search and symbol analysis", vec![]);
        let out = resp.render();
        assert!(
            out.starts_with("tool:        my-search-tool\ndescription: Semantic code search and symbol analysis\n"),
            "got: {out}"
        );
    }

    #[test]
    fn ok_health_item_has_no_bracket_annotation() {
        let resp = StatusResponse::new(
            "t",
            "d",
            vec![StatusItem { key: "index".into(), value: KvValue::Text("ready".into()), health: Some(Health::Ok) }],
        );
        let out = resp.render();
        assert!(out.contains("index:       ready\n"), "got: {out}");
        assert!(!out.contains("[OK"));
    }

    #[test]
    fn degraded_item_gets_bracket_annotation() {
        let resp = StatusResponse::new(
            "t",
            "d",
            vec![StatusItem {
                key: "cache".into(),
                value: KvValue::Text("warm (98MB / 100MB)".into()),
                health: Some(Health::Degraded("approaching limit".into())),
            }],
        );
        let out = resp.render();
        assert!(out.contains("cache:       warm (98MB / 100MB)  [DEGRADED: approaching limit]\n"), "got: {out}");
    }

    #[test]
    fn error_item_gets_bracket_annotation() {
        let resp = StatusResponse::new(
            "t",
            "d",
            vec![StatusItem {
                key: "queue".into(),
                value: KvValue::Text("stalled".into()),
                health: Some(Health::Error("connection lost".into())),
            }],
        );
        let out = resp.render();
        assert!(out.contains("[ERROR: connection lost]"));
    }

    #[test]
    fn item_with_no_health_renders_plain() {
        let resp = StatusResponse::new(
            "t",
            "d",
            vec![StatusItem { key: "files".into(), value: KvValue::Int(2847), health: None }],
        );
        let out = resp.render();
        assert!(out.contains("files:       2847\n"), "got: {out}");
    }

    #[test]
    fn hints_append_help_block() {
        let resp = StatusResponse::new("t", "d", vec![]).with_hints(vec![Hint::new("Run `search <query>` to search")]);
        assert!(resp.render().contains("help[1]:\n  Run `search <query>` to search\n"));
    }

    #[test]
    fn full_example_matches_spec() {
        let resp = StatusResponse::new(
            "my-search-tool",
            "Semantic code search and symbol analysis",
            vec![
                StatusItem { key: "index".into(), value: KvValue::Text("ready".into()), health: Some(Health::Ok) },
                StatusItem { key: "files".into(), value: KvValue::Int(2847), health: None },
                StatusItem {
                    key: "cache".into(),
                    value: KvValue::Text("warm (98MB / 100MB)".into()),
                    health: Some(Health::Degraded("approaching limit".into())),
                },
                StatusItem { key: "last-updated".into(), value: KvValue::Text("4 minutes ago".into()), health: None },
            ],
        )
        .with_hints(vec![Hint::new("Run `search <query>` to search")]);
        let out = resp.render();
        // NOTE: the exact spacing below may not match this crate's actual +1 padding formula
        // (verified/fixed in Task 10 — see kv::mod.rs). Compute the expected string by RUNNING
        // the code and reading its actual output, then hand-verify the padding arithmetic against
        // kv::render_kv's `pad = max_key_len - key_len + 1` formula (longest key here is
        // "last-updated" at 12 chars) BEFORE trusting either this literal or your computed one.
        let expected = "tool:         my-search-tool\ndescription:  Semantic code search and symbol analysis\nindex:        ready\nfiles:        2847\ncache:        warm (98MB / 100MB)  [DEGRADED: approaching limit]\nlast-updated: 4 minutes ago\nhelp[1]:\n  Run `search <query>` to search\n";
        assert_eq!(out, expected);
    }
}
