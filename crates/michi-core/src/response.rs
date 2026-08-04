use crate::hints::Hint;
use crate::kv::KvItem;
use crate::recovery::RecoveryHint;
use michi_toon::Value;

/// The serialisation format for `AgentResponse::render`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum OutputFormat {
    /// Plain-text TOON / kv format. Default.
    #[default]
    Text,
    /// Compact JSON object.
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RenderTarget {
    Unset,
    Toon,
    Kv,
}

/// Builder for an agent-facing response.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct AgentResponse {
    type_name: String,
    target: RenderTarget,
    items: Vec<Vec<Value>>,
    fields: Vec<String>,
    single_item: Vec<KvItem>,
    total_count: Option<usize>,
    hints: Vec<Hint>,
    recovery: Vec<RecoveryHint>,
    truncate_cells_at: usize,
    is_error: bool,
    human_content: Option<String>,
}

impl AgentResponse {
    /// Create a new, empty response for the given type name.
    pub fn new(type_name: impl Into<String>) -> Self {
        Self {
            type_name: type_name.into(),
            target: RenderTarget::Unset,
            items: Vec::new(),
            fields: Vec::new(),
            single_item: Vec::new(),
            total_count: None,
            hints: Vec::new(),
            recovery: Vec::new(),
            truncate_cells_at: 200,
            is_error: false,
            human_content: None,
        }
    }

    /// Populate the TOON list path.
    #[must_use]
    pub fn items(mut self, rows: Vec<Vec<Value>>, fields: &[&str]) -> Self {
        self.items = rows;
        self.fields = fields.iter().map(|s| (*s).to_string()).collect();
        self.target = RenderTarget::Toon;
        self
    }

    /// Set the total available count.
    #[must_use]
    pub fn total_count(mut self, n: usize) -> Self {
        self.total_count = Some(n);
        self
    }

    /// Populate the KV single-item path.
    #[must_use]
    pub fn kv_items(mut self, items: Vec<KvItem>) -> Self {
        self.single_item = items;
        self.target = RenderTarget::Kv;
        self
    }

    /// Append a contextual hint.
    #[must_use]
    pub fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hints.push(Hint::new(hint));
        self
    }

    /// Replace all contextual hints.
    #[must_use]
    pub fn hints(mut self, hints: Vec<Hint>) -> Self {
        self.hints = hints;
        self
    }

    /// Append a recovery hint.
    #[must_use]
    pub fn recovery_hint(mut self, r: RecoveryHint) -> Self {
        self.recovery.push(r);
        self
    }

    /// Set the max cell length before inline truncation on the TOON path.
    #[must_use]
    pub fn truncate_cells_at(mut self, limit: usize) -> Self {
        self.truncate_cells_at = limit;
        self
    }

    /// Mark this response as an error state.
    #[must_use]
    pub fn as_error(mut self) -> Self {
        self.is_error = true;
        self
    }

    /// Attach a human-facing companion block (`audience: user`).
    #[must_use]
    pub fn human_content(mut self, text: impl Into<String>) -> Self {
        self.human_content = Some(text.into());
        self
    }

    fn toon_body(&self) -> String {
        let opts = michi_toon::ToonOptions::new(self.type_name.clone(), self.fields.clone(), self.items.clone())
            .total_count(self.total_count)
            .max_cell_len(self.truncate_cells_at);
        if let Err(e) = opts.validate() {
            return format!("error: toon_validation_failed\nmessage: {e}\n");
        }
        michi_toon::render_toon(&opts)
    }

    fn kv_body(&self) -> String {
        crate::kv::render_kv(&self.single_item, self.total_count, &[])
    }

    fn body(&self) -> String {
        match self.target {
            RenderTarget::Toon | RenderTarget::Unset => self.toon_body(),
            RenderTarget::Kv => self.kv_body(),
        }
    }

    fn finish_text(&self, body: &str) -> String {
        let mut out = String::with_capacity(body.len() + self.hints.len() * 60 + self.recovery.len() * 80);
        out.push_str(body);
        crate::hints::append_hints(&mut out, &self.hints);
        crate::recovery::append_recovery(&mut out, &self.recovery);
        out
    }

    /// Render the response in the requested format.
    #[must_use]
    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => self.render_text(),
            OutputFormat::Json => self.render_json(),
        }
    }

    /// Render via the TOON path.
    #[must_use]
    pub fn render_toon(&self) -> String {
        self.finish_text(&self.toon_body())
    }

    /// Render via the KV path.
    #[must_use]
    pub fn render_kv(&self) -> String {
        self.finish_text(&self.kv_body())
    }

    fn render_text(&self) -> String {
        self.finish_text(&self.body())
    }

    /// Render just the `help[N]:` block for `self.hints`.
    #[must_use]
    pub fn render_hints_only(&self) -> String {
        crate::hints::render_hints(&self.hints)
    }

    /// Render for the given audience.
    #[must_use]
    pub fn render_for(&self, audience: crate::audience::Audience) -> String {
        match audience {
            crate::audience::Audience::Assistant => self.render_text(),
            crate::audience::Audience::User => self.human_content.clone().unwrap_or_else(|| self.render_text()),
        }
    }

    /// Whether `.human_content()` was set on this builder.
    #[must_use]
    pub fn has_human_content(&self) -> bool {
        self.human_content.is_some()
    }

    /// Build the MCP `CallToolResult` for this response.
    #[must_use]
    pub fn to_call_tool_result(&self) -> crate::mcp::CallToolResult {
        let mut content = vec![crate::mcp::ContentBlock {
            text: self.render_text(),
            audience: vec![crate::audience::Audience::Assistant],
        }];
        if let Some(human) = &self.human_content {
            content.push(crate::mcp::ContentBlock {
                text: human.clone(),
                audience: vec![crate::audience::Audience::User],
            });
        }
        crate::mcp::CallToolResult {
            content,
            is_error: self.is_error,
            structured_content: self.render(OutputFormat::Json),
        }
    }

    fn render_json(&self) -> String {
        let body = self.body();
        let capacity = body.len()
            + self.hints.iter().map(|h| h.as_str().len() + 16).sum::<usize>()
            + self.recovery.len() * 64
            + 64;
        let mut out = String::with_capacity(capacity);
        out.push_str("{\"body\":");
        json_string(&mut out, &body);
        out.push_str(",\"hints\":[");
        for (i, h) in self.hints.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            json_string(&mut out, h.as_str());
        }
        out.push_str("],\"recovery\":[");
        for (i, r) in self.recovery.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str("{\"tool\":");
            json_string(&mut out, &r.tool);
            out.push_str(",\"params\":{");
            for (j, (k, v)) in r.params.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                json_string(&mut out, k);
                out.push(':');
                crate::kv::kv_value_to_json(&mut out, v);
            }
            out.push('}');
            if let Some(reason) = &r.reason {
                out.push_str(",\"reason\":");
                json_string(&mut out, reason);
            }
            out.push('}');
        }
        out.push_str("],\"isError\":");
        out.push_str(if self.is_error { "true" } else { "false" });
        out.push('}');
        out
    }
}

fn json_string(out: &mut String, s: &str) {
    crate::kv::json_escape_str(out, s);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn items_path_renders_toon() {
        let r = AgentResponse::new("issues").items(vec![vec![Value::Int(1), Value::from("open")]], &["id", "state"]);
        let out = r.render(OutputFormat::Text);
        assert!(out.starts_with("issues[1]{id,state}:\n  1,open\n"), "got: {out}");
    }

    #[test]
    fn render_json_basic_structure() {
        let r = AgentResponse::new("items").items(vec![vec![Value::from("hello world")]], &["name"]);
        let json = r.render_json();
        assert!(json.contains("\"isError\":false"), "got: {json}");
        assert!(json.contains("\"body\""), "got: {json}");
        assert!(json.contains("\"hints\":[]"), "got: {json}");
    }

    #[test]
    fn render_json_is_error_flag() {
        let r = AgentResponse::new("items").items(vec![vec![Value::from("fail")]], &["name"]).as_error();
        let json = r.render_json();
        assert!(json.contains("\"isError\":true"), "got: {json}");
    }

    #[test]
    fn render_json_recovery_int_param_is_numeric_not_string() {
        use crate::kv::KvValue;
        use crate::recovery::RecoveryHint;
        let recovery = RecoveryHint::new("retry").param("limit", KvValue::Int(10));
        let r =
            AgentResponse::new("items").items(vec![vec![Value::from("hit limit")]], &["name"]).recovery_hint(recovery);
        let json = r.render_json();
        assert!(json.contains("\"limit\":10"), "Int param must be unquoted, got: {json}");
        assert!(!json.contains("\"limit\":\"10\""), "Int param must not be quoted string, got: {json}");
    }

    #[test]
    fn render_json_hint_with_quotes_is_escaped() {
        let r = AgentResponse::new("items").hint(r#"Use "get_item" instead"#);
        let json = r.render_json();
        assert!(json.contains("\\\"get_item\\\""), "quotes in hints must be escaped, got: {json}");
    }

    #[test]
    fn to_call_tool_result_composes_correctly() {
        let r = AgentResponse::new("items").items(vec![vec![Value::from("ok")]], &["name"]).as_error();
        let result = r.to_call_tool_result();
        assert!(result.is_error);
        assert!(!result.content.is_empty(), "content must not be empty");
        assert!(result.structured_content.starts_with('{'), "structured_content must be JSON object");
    }

    #[test]
    fn toon_body_returns_error_on_invalid_type_name() {
        let r = AgentResponse::new("items[bad]").items(vec![vec![Value::from("x")]], &["name"]);
        let out = r.render(OutputFormat::Text);
        assert!(out.contains("error:"), "expected error output for invalid type_name, got: {out}");
    }

    #[test]
    fn render_hints_only_contains_help_block() {
        let r = AgentResponse::new("items").hint("Try again with fewer params");
        let out = r.render_hints_only();
        assert!(out.contains("help[1]:"), "expected help block, got: {out}");
        assert!(out.contains("Try again with fewer params"), "got: {out}");
    }

    #[test]
    fn render_hints_only_empty_when_no_hints() {
        let r = AgentResponse::new("items");
        assert!(r.render_hints_only().is_empty(), "no hints should produce empty string");
    }

    #[test]
    fn render_for_assistant_returns_text_output() {
        let r = AgentResponse::new("items").items(vec![vec![Value::from("ok")]], &["name"]);
        let out = r.render_for(crate::audience::Audience::Assistant);
        assert!(!out.is_empty(), "assistant render must not be empty");
        assert!(out.contains("items"), "got: {out}");
    }

    #[test]
    fn render_for_user_returns_human_content_when_set() {
        let r = AgentResponse::new("items").human_content("Human-readable summary.");
        let out = r.render_for(crate::audience::Audience::User);
        assert_eq!(out, "Human-readable summary.");
    }

    #[test]
    fn render_for_user_falls_back_to_text_when_no_human_content() {
        let r = AgentResponse::new("items").items(vec![vec![Value::from("x")]], &["name"]);
        let assistant_out = r.render_for(crate::audience::Audience::Assistant);
        let user_out = r.render_for(crate::audience::Audience::User);
        assert_eq!(assistant_out, user_out, "user render must match assistant render when no human_content set");
    }

    #[test]
    fn has_human_content_false_when_empty() {
        let r = AgentResponse::new("items");
        assert!(!r.has_human_content());
    }

    #[test]
    fn has_human_content_true_when_set() {
        let r = AgentResponse::new("items").human_content("Some text for humans.");
        assert!(r.has_human_content());
    }
}
