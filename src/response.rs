use crate::hints::Hint;
use crate::kv::KvItem;
use crate::recovery::RecoveryHint;
use crate::toon::Value;

/// The serialisation format for `AgentResponse::render`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain-text TOON / kv format. Default.
    #[default]
    Text,
    /// Compact JSON object — field names match the builder setters.
    Json,
}

/// Which underlying format an `AgentResponse` will render as, determined by
/// which of `.items()` / `.kv_items()` was called (not by item count).
#[derive(Debug, Clone, PartialEq, Eq)]
enum RenderTarget {
    /// Neither `.items()` nor `.kv_items()` called yet.
    Unset,
    /// `.items()` called — routes to `toon::render_toon`.
    Toon,
    /// `.kv_items()` called — routes to `kv::render_kv`.
    Kv,
}

/// Builder for an agent-facing response. Routes to TOON or KV based on which
/// items method is called, not on item count.
///
/// # Format routing
/// - `.items()` called    → TOON (list of uniform-schema rows)
/// - `.kv_items()` called → KV   (single item or mixed-type metadata)
///
/// Use `.items()` for 5+ uniform rows. Use `.kv_items()` for single items,
/// status data, or heterogeneous metadata. Calling both on one builder is a
/// caller-side logic error; treat one `AgentResponse` as one output shape.
/// `render(format)` dispatches on whichever of `.items()`/`.kv_items()` was
/// called *last*. The [`Self::render_toon`]/[`Self::render_kv`] shorthands are
/// slot-specific instead: each always reads its own named slot
/// (`items`/`fields` or `single_item`), independent of which method was
/// called last.
///
/// # Examples
///
/// ```rust
/// use michi::response::{AgentResponse, OutputFormat};
///
/// let out = AgentResponse::new("issues")
///     .items(vec![], &["number", "title"])
///     .hint("Try a broader filter")
///     .render(OutputFormat::Text);
/// assert!(out.contains("help[1]:"));
/// ```
#[derive(Debug, Clone)]
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
    /// Create a new, empty response for the given type name. Neither
    /// `.items()` nor `.kv_items()` has been called yet — rendering an unset
    /// response produces an empty-ish TOON header for `type_name` (via the
    /// same path as an empty `.items()` call), since that's a safer default
    /// than panicking on a builder a caller hasn't finished configuring.
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

    /// Populate the TOON list path. Routes rendering to `toon::render_toon`.
    #[must_use]
    pub fn items(mut self, rows: Vec<Vec<Value>>, fields: &[&str]) -> Self {
        self.items = rows;
        self.fields = fields.iter().map(|s| (*s).to_string()).collect();
        self.target = RenderTarget::Toon;
        self
    }

    /// Set the total available count, emitted as `totalCount: N` on either
    /// render path — the TOON header line and the KV block both honour it.
    #[must_use]
    pub fn total_count(mut self, n: usize) -> Self {
        self.total_count = Some(n);
        self
    }

    /// Populate the KV single-item path. Routes rendering to `kv::render_kv`.
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

    /// Set the max cell length before inline truncation on the TOON path (see
    /// `toon::ToonOptions::max_cell_len`). Default: 200.
    #[must_use]
    pub fn truncate_cells_at(mut self, limit: usize) -> Self {
        self.truncate_cells_at = limit;
        self
    }

    /// Mark this response as an error state — reflected in
    /// `OutputFormat::Json`'s `isError` field.
    #[must_use]
    pub fn as_error(mut self) -> Self {
        self.is_error = true;
        self
    }

    /// Attach a human-facing companion block (`audience: user`) for MCP
    /// callers. Optional — most callers won't set this. michi does not
    /// generate this text itself; the caller supplies it.
    #[must_use]
    pub fn human_content(mut self, text: impl Into<String>) -> Self {
        self.human_content = Some(text.into());
        self
    }

    /// Render the TOON slot (`items`/`fields`/`total_count`) regardless of
    /// `target`. Backs both the `target`-based `body()` dispatch and the
    /// slot-specific [`Self::render_toon`] shorthand.
    fn toon_body(&self) -> String {
        let opts = crate::toon::ToonOptions {
            type_name: self.type_name.clone(),
            fields: self.fields.clone(),
            rows: self.items.clone(),
            total_count: self.total_count,
            hints: Vec::new(), // hints are appended once, below, not duplicated into the TOON body
            max_cell_len: self.truncate_cells_at,
        };
        crate::toon::render_toon(&opts)
    }

    /// Render the KV slot (`single_item`/`total_count`) regardless of
    /// `target`. Backs both the `target`-based `body()` dispatch and the
    /// slot-specific [`Self::render_kv`] shorthand.
    fn kv_body(&self) -> String {
        crate::kv::render_kv(&self.single_item, self.total_count, &[])
    }

    fn body(&self) -> String {
        match self.target {
            RenderTarget::Toon | RenderTarget::Unset => self.toon_body(),
            RenderTarget::Kv => self.kv_body(),
        }
    }

    /// Append `hints`/`recovery` after an already-rendered body — shared tail
    /// for `render_text()` and the slot-specific `render_toon()`/`render_kv()`.
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

    /// Render via the TOON path (`items`/`fields`/`total_count`), reading
    /// only that slot regardless of `target` — i.e. regardless of whether
    /// `.items()` or `.kv_items()` was called last. Unlike `render(format)`,
    /// which dispatches on `target`, this method's output never changes based
    /// on `.kv_items()` having been called.
    #[must_use]
    pub fn render_toon(&self) -> String {
        self.finish_text(&self.toon_body())
    }

    /// Render via the KV path (`single_item`/`total_count`), reading only
    /// that slot regardless of `target` — the KV-path counterpart of
    /// [`Self::render_toon`].
    #[must_use]
    pub fn render_kv(&self) -> String {
        self.finish_text(&self.kv_body())
    }

    fn render_text(&self) -> String {
        self.finish_text(&self.body())
    }

    /// Render just the `help[N]:` block for `self.hints` — the three-surface
    /// seam for MCP frameworks that render a display body separately (via
    /// their own Markdown layer) and only need michi to own the `help[]`
    /// format for the agent-facing content block. Returns an empty string
    /// when there are no hints.
    #[must_use]
    pub fn render_hints_only(&self) -> String {
        crate::hints::render_hints(&self.hints)
    }

    /// Build the MCP `CallToolResult` for this response: the rendered body
    /// as the primary `assistant`-audience content block, `human_content`
    /// (if set) as a second `user`-audience block, `is_error`, and
    /// `structured_content` as the JSON-rendered form of the same data.
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

/// Append a JSON-encoded string (with surrounding quotes and escape sequences)
/// to `out`. Delegates to [`crate::kv::json_escape_str`], which also backs
/// [`crate::kv::kv_value_to_json`] — one escaper, not two copies of the same
/// RFC 8259 control-character handling.
fn json_string(out: &mut String, s: &str) {
    crate::kv::json_escape_str(out, s);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::{KvItem, KvValue};
    use crate::toon::Value;

    #[test]
    fn items_path_renders_toon() {
        let r =
            AgentResponse::new("issues").items(vec![vec![Value::Int(1), Value::Str("open".into())]], &["id", "state"]);
        let out = r.render(OutputFormat::Text);
        assert!(out.starts_with("issues[1]{id,state}:\n  1,open\n"), "got: {out}");
    }

    #[test]
    fn kv_items_path_renders_kv() {
        let r = AgentResponse::new("issue").kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(42) }]);
        let out = r.render(OutputFormat::Text);
        assert_eq!(out, "id: 42\n");
    }

    #[test]
    fn total_count_appears_in_toon_output() {
        let r = AgentResponse::new("issues").items(vec![], &["id"]).total_count(99);
        assert!(r.render(OutputFormat::Text).contains("totalCount: 99"));
    }

    #[test]
    fn total_count_appears_in_kv_output_too() {
        let r = AgentResponse::new("issue")
            .kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(1) }])
            .total_count(5);
        assert_eq!(r.render(OutputFormat::Text), "id: 1\ntotalCount: 5\n");
    }

    #[test]
    fn unset_target_renders_as_empty_toon_header_for_type_name() {
        let r = AgentResponse::new("issue");
        assert_eq!(r.render(OutputFormat::Text), "issue[0]{}:\n");
    }

    #[test]
    fn hint_and_recovery_append_after_body() {
        let r = AgentResponse::new("issue")
            .kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(1) }])
            .hint("do this")
            .recovery_hint(RecoveryHint::new("retry"));
        let out = r.render(OutputFormat::Text);
        let hint_pos = out.find("help[").unwrap();
        let recovery_pos = out.find("recovery[").unwrap();
        assert!(hint_pos < recovery_pos);
    }

    #[test]
    fn truncate_cells_at_applies_to_toon_items() {
        let long = "x".repeat(300);
        let r = AgentResponse::new("t").items(vec![vec![Value::Str(long)]], &["field"]).truncate_cells_at(50);
        assert!(r.render(OutputFormat::Text).contains("chars truncated"));
    }

    #[test]
    fn as_error_sets_flag_in_json() {
        let r = AgentResponse::new("t").kv_items(vec![]).as_error();
        assert!(r.render(OutputFormat::Json).contains("\"isError\":true"));
    }

    #[test]
    fn json_format_omits_hints_and_recovery_keys_when_empty_toon() {
        let r = AgentResponse::new("issues").items(vec![], &["id"]);
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("\"isError\":false"));
    }

    #[test]
    fn render_hints_only_returns_just_the_help_block() {
        let r = AgentResponse::new("t").kv_items(vec![]).hint("call foo").hint("call bar");
        assert_eq!(r.render_hints_only(), "help[2]:\n  call foo\n  call bar\n");
    }

    #[test]
    fn render_hints_only_empty_when_no_hints() {
        let r = AgentResponse::new("t").kv_items(vec![]);
        assert_eq!(r.render_hints_only(), "");
    }

    #[test]
    fn render_toon_shorthand_matches_render_text_with_toon_format() {
        let r = AgentResponse::new("issues").items(vec![vec![Value::Int(1)]], &["id"]);
        assert_eq!(r.render_toon(), r.render(OutputFormat::Text));
    }

    #[test]
    fn render_kv_shorthand_matches_render_text_with_kv_format() {
        let r = AgentResponse::new("issue").kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(1) }]);
        assert_eq!(r.render_kv(), r.render(OutputFormat::Text));
    }

    #[test]
    fn render_toon_reads_toon_slot_even_when_kv_items_called_last() {
        // `.kv_items()` is called last, so `target` (and thus `render(format)`)
        // would follow the KV path — but `render_toon()` must still read the
        // `items`/`fields` slot regardless.
        let r = AgentResponse::new("issues")
            .items(vec![vec![Value::Int(1)]], &["id"])
            .kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(99) }]);
        let toon = r.render_toon();
        assert!(toon.starts_with("issues[1]{id}:\n  1\n"), "got: {toon}");
        assert_ne!(toon, r.render_kv(), "render_toon() must not equal render_kv()'s output");
        assert_eq!(
            r.render_kv(),
            r.render(OutputFormat::Text),
            "target is Kv, so render(format) follows render_kv() here, not render_toon()"
        );
    }

    #[test]
    fn render_kv_reads_kv_slot_even_when_items_called_last() {
        // `.items()` is called last, so `target` (and thus `render(format)`)
        // would follow the TOON path — but `render_kv()` must still read the
        // `single_item` slot regardless.
        let r = AgentResponse::new("issue")
            .kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(1) }])
            .items(vec![vec![Value::Int(99)]], &["id"]);
        let kv = r.render_kv();
        assert_eq!(kv, "id: 1\n");
        assert_ne!(kv, r.render_toon(), "render_kv() must not equal render_toon()'s output");
        assert_eq!(
            r.render_toon(),
            r.render(OutputFormat::Text),
            "target is Toon, so render(format) follows render_toon() here, not render_kv()"
        );
    }

    #[test]
    fn multiple_hint_calls_accumulate() {
        let r = AgentResponse::new("t").kv_items(vec![]).hint("a").hint("b");
        assert!(r.render_hints_only().contains("help[2]:"));
    }

    #[test]
    fn render_json_recovery_params_use_native_json_types_not_strings() {
        let r = AgentResponse::new("t").kv_items(vec![]).recovery_hint(
            RecoveryHint::new("retry_after")
                .param("seconds", KvValue::Int(30))
                .param("force", KvValue::Bool(true))
                .param("ratio", KvValue::Float(0.5, 2)),
        );
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("\"seconds\":30"), "got: {json}");
        assert!(json.contains("\"force\":true"), "got: {json}");
        assert!(json.contains("\"ratio\":0.50"), "got: {json}");
        assert!(!json.contains("\"seconds\":\"30\""), "int must not be JSON-stringified, got: {json}");
        assert!(!json.contains("\"force\":\"true\""), "bool must not be JSON-stringified, got: {json}");
    }

    #[test]
    fn render_json_recovery_text_param_is_a_quoted_json_string() {
        let r = AgentResponse::new("t")
            .kv_items(vec![])
            .recovery_hint(RecoveryHint::new("assign_user").param("user", KvValue::Text("alice".into())));
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("\"user\":\"alice\""), "got: {json}");
    }

    #[test]
    fn render_json_recovery_missing_param_is_json_null() {
        let r = AgentResponse::new("t")
            .kv_items(vec![])
            .recovery_hint(RecoveryHint::new("t").param("value", KvValue::Missing));
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("\"value\":null"), "got: {json}");
    }

    #[test]
    fn to_call_tool_result_uses_render_text_as_assistant_block() {
        let r = AgentResponse::new("issue").kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(1) }]);
        let result = r.to_call_tool_result();
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, r.render_kv());
        assert_eq!(result.content[0].audience, vec![crate::audience::Audience::Assistant]);
        assert!(!result.is_error);
    }

    #[test]
    fn to_call_tool_result_uses_render_text_as_assistant_block_for_toon_path() {
        let r = AgentResponse::new("issue")
            .items(vec![vec![Value::Int(1), Value::Str("open".to_string())]], &["id", "state"]);
        let result = r.to_call_tool_result();
        assert_eq!(result.content.len(), 1);
        assert_eq!(result.content[0].text, r.render_toon());
        assert_eq!(result.content[0].audience, vec![crate::audience::Audience::Assistant]);
    }

    #[test]
    fn to_call_tool_result_reflects_is_error() {
        let r = AgentResponse::new("t").kv_items(vec![]).as_error();
        let result = r.to_call_tool_result();
        assert!(result.is_error);
    }

    #[test]
    fn to_call_tool_result_includes_human_content_block_when_set() {
        let r = AgentResponse::new("t").kv_items(vec![]).human_content("Here's a friendly summary.");
        let result = r.to_call_tool_result();
        assert_eq!(result.content.len(), 2);
        assert_eq!(result.content[1].text, "Here's a friendly summary.");
        assert_eq!(result.content[1].audience, vec![crate::audience::Audience::User]);
    }

    #[test]
    fn to_call_tool_result_omits_human_block_when_not_set() {
        let r = AgentResponse::new("t").kv_items(vec![]);
        let result = r.to_call_tool_result();
        assert_eq!(result.content.len(), 1);
    }

    #[test]
    fn to_call_tool_result_structured_content_is_valid_json_matching_render_json_body() {
        let r = AgentResponse::new("issue").kv_items(vec![KvItem { key: "id".into(), value: KvValue::Int(1) }]);
        let result = r.to_call_tool_result();
        // structured_content is the same JSON render() produces for OutputFormat::Json.
        assert_eq!(result.structured_content, r.render(OutputFormat::Json));
    }
}
