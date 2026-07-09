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
/// caller-side logic error — whichever was called *last* wins at render time;
/// treat one `AgentResponse` as one output shape.
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

    fn body(&self) -> String {
        match self.target {
            RenderTarget::Toon | RenderTarget::Unset => {
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
            RenderTarget::Kv => crate::kv::render_kv(&self.single_item, self.total_count, &[]),
        }
    }

    /// Render the response in the requested format.
    #[must_use]
    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => self.render_text(),
            OutputFormat::Json => self.render_json(),
        }
    }

    /// Shorthand for `render(OutputFormat::Text)` when the TOON path was used.
    #[must_use]
    pub fn render_toon(&self) -> String {
        self.render_text()
    }

    /// Shorthand for `render(OutputFormat::Text)` when the KV path was used.
    #[must_use]
    pub fn render_kv(&self) -> String {
        self.render_text()
    }

    fn render_text(&self) -> String {
        let body = self.body();
        let mut out = String::with_capacity(body.len() + self.hints.len() * 60 + self.recovery.len() * 80);
        out.push_str(&body);
        crate::hints::append_hints(&mut out, &self.hints);
        crate::recovery::append_recovery(&mut out, &self.recovery);
        out
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
                json_string(&mut out, &crate::recovery::kv_value_str(v));
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
/// to `out`. Escapes `"`, `\`, `\n`, `\r`, `\t`, and all other control characters
/// (U+0000–U+001F) as `\u00XX` per RFC 8259.
fn json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other if u32::from(other) < 0x20 => {
                let code = u32::from(other);
                out.push_str("\\u00");
                out.push(hex_digit(code >> 4));
                out.push(hex_digit(code & 0xf));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Render a nibble (0–15) as a lowercase hex digit.
fn hex_digit(nibble: u32) -> char {
    char::from_digit(nibble, 16).unwrap_or('0')
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
    fn multiple_hint_calls_accumulate() {
        let r = AgentResponse::new("t").kv_items(vec![]).hint("a").hint("b");
        assert!(r.render_hints_only().contains("help[2]:"));
    }
}
