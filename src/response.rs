use crate::hints::Hint;
use crate::recovery::RecoveryHint;

/// The serialisation format for `AgentResponse::render`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain-text TOON / kv / status format. Default.
    #[default]
    Text,
    /// Compact JSON object — field names match the builder setters.
    Json,
}

/// Builder for composing a complete agent response from michi primitives.
///
/// An `AgentResponse` collects a primary content body, optional recovery hints,
/// optional contextual hints, and metadata, then renders the whole thing as a
/// single string via [`AgentResponse::render`].
///
/// # Examples
///
/// ```rust
/// use michi::response::{AgentResponse, OutputFormat};
/// use michi::hints::Hint;
///
/// let out = AgentResponse::new("issue[0]{}:\ntotalCount: 0\n")
///     .with_hint(Hint::new("Try a broader filter"))
///     .render(OutputFormat::Text);
/// assert!(out.contains("help[1]:"));
/// ```
#[derive(Debug, Clone)]
pub struct AgentResponse {
    /// Primary content body (TOON block, kv block, status block, etc.).
    pub body: String,
    /// Contextual hints appended after `body`.
    pub hints: Vec<Hint>,
    /// Structured recovery hints appended after `hints`.
    pub recovery: Vec<RecoveryHint>,
    /// Whether the response represents an error state.
    pub is_error: bool,
}

impl AgentResponse {
    /// Create a new response with the given body. Hints and recovery are empty.
    pub fn new(body: impl Into<String>) -> Self {
        Self { body: body.into(), hints: Vec::new(), recovery: Vec::new(), is_error: false }
    }

    /// Create a new response that represents an error state.
    pub fn error(body: impl Into<String>) -> Self {
        Self { body: body.into(), hints: Vec::new(), recovery: Vec::new(), is_error: true }
    }

    /// Append a contextual hint.
    #[must_use]
    pub fn with_hint(mut self, hint: impl Into<Hint>) -> Self {
        self.hints.push(hint.into());
        self
    }

    /// Replace all contextual hints.
    #[must_use]
    pub fn with_hints(mut self, hints: Vec<Hint>) -> Self {
        self.hints = hints;
        self
    }

    /// Append a recovery hint.
    #[must_use]
    pub fn with_recovery(mut self, hint: RecoveryHint) -> Self {
        self.recovery.push(hint);
        self
    }

    /// Replace all recovery hints.
    #[must_use]
    pub fn with_recoveries(mut self, hints: Vec<RecoveryHint>) -> Self {
        self.recovery = hints;
        self
    }

    /// Mark this response as an error state.
    #[must_use]
    pub fn as_error(mut self) -> Self {
        self.is_error = true;
        self
    }

    /// Render the response in the requested format.
    ///
    /// **Text format:** `body` + optional `help[N]:` block + optional `recovery[N]:` block.
    ///
    /// **Json format:** `{"body":"...","hints":[...],"recovery":[{"action":"...","description":"...","example":"..."}],"isError":false}`
    #[must_use]
    pub fn render(&self, format: OutputFormat) -> String {
        match format {
            OutputFormat::Text => self.render_text(),
            OutputFormat::Json => self.render_json(),
        }
    }

    fn render_text(&self) -> String {
        let capacity = self.body.len() + self.hints.len() * 60 + self.recovery.len() * 80;
        let mut out = String::with_capacity(capacity);
        out.push_str(&self.body);
        crate::hints::append_hints(&mut out, &self.hints);
        crate::recovery::append_recovery(&mut out, &self.recovery);
        out
    }

    fn render_json(&self) -> String {
        // Hand-built JSON — no serde dep in default features.
        let capacity = self.body.len()
            + self.hints.iter().map(|h| h.as_str().len() + 16).sum::<usize>()
            + self.recovery.len() * 64
            + 64;
        let mut out = String::with_capacity(capacity);
        out.push_str("{\"body\":");
        json_string(&mut out, &self.body);
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
            out.push_str("{\"action\":");
            json_string(&mut out, &r.action);
            out.push_str(",\"description\":");
            json_string(&mut out, &r.description);
            if let Some(ex) = &r.example {
                out.push_str(",\"example\":");
                json_string(&mut out, ex);
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
    use crate::recovery::RecoveryHint;

    #[test]
    fn text_render_body_only() {
        let r = AgentResponse::new("issue[0]{}:\ntotalCount: 0\n");
        assert_eq!(r.render(OutputFormat::Text), "issue[0]{}:\ntotalCount: 0\n");
    }

    #[test]
    fn text_render_with_hints() {
        let r = AgentResponse::new("body\n").with_hint(Hint::new("do this"));
        let out = r.render(OutputFormat::Text);
        assert_eq!(out, "body\nhelp[1]:\n  do this\n");
    }

    #[test]
    fn text_render_with_recovery() {
        let r = AgentResponse::new("err\n").with_recovery(RecoveryHint::new("retry", "wait and retry"));
        let out = r.render(OutputFormat::Text);
        assert_eq!(out, "err\nrecovery[1]:\n  retry: wait and retry\n");
    }

    #[test]
    fn text_render_with_hints_and_recovery_combined() {
        let r = AgentResponse::new("body\n")
            .with_hint(Hint::new("hint one"))
            .with_recovery(RecoveryHint::new("retry", "wait and retry"));
        let out = r.render(OutputFormat::Text);
        assert_eq!(out, "body\nhelp[1]:\n  hint one\nrecovery[1]:\n  retry: wait and retry\n");
    }

    #[test]
    fn text_render_hints_before_recovery() {
        let r = AgentResponse::new("body\n")
            .with_hint(Hint::new("hint"))
            .with_recovery(RecoveryHint::new("retry", "try again"));
        let out = r.render(OutputFormat::Text);
        let hint_pos = out.find("help[").unwrap();
        let recovery_pos = out.find("recovery[").unwrap();
        assert!(hint_pos < recovery_pos, "hints must precede recovery");
    }

    #[test]
    fn json_render_basic() {
        let r = AgentResponse::new("hello");
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("\"body\":\"hello\""));
        assert!(json.contains("\"hints\":[]"));
        assert!(json.contains("\"recovery\":[]"));
        assert!(json.contains("\"isError\":false"));
    }

    #[test]
    fn json_render_error_flag() {
        let r = AgentResponse::error("bad").as_error();
        assert!(r.render(OutputFormat::Json).contains("\"isError\":true"));
    }

    #[test]
    fn json_render_escapes_quotes() {
        let r = AgentResponse::new("say \"hello\"");
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("say \\\"hello\\\""));
    }

    #[test]
    fn json_string_escapes_control_chars_as_unicode_escapes() {
        let mut out = String::new();
        json_string(&mut out, "\u{0}\u{1}\u{7}\u{1f}");
        assert_eq!(out, "\"\\u0000\\u0001\\u0007\\u001f\"");
    }

    #[test]
    fn json_string_named_escapes_are_not_double_escaped() {
        let mut out = String::new();
        json_string(&mut out, "\n\r\t\"\\");
        assert_eq!(out, "\"\\n\\r\\t\\\"\\\\\"");
    }

    #[test]
    fn json_render_escapes_bel_control_char() {
        let r = AgentResponse::new("hello\u{7}world");
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("hello\\u0007world"), "BEL char must be escaped, got: {json}");
    }

    #[test]
    fn json_render_with_hints() {
        let r = AgentResponse::new("body").with_hint(Hint::new("use list"));
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("\"hints\":[\"use list\"]"));
    }

    #[test]
    fn json_render_with_recovery() {
        let r = AgentResponse::new("body").with_recovery(RecoveryHint::with_example("retry", "wait 1s", "retry()"));
        let json = r.render(OutputFormat::Json);
        assert!(json.contains("\"action\":\"retry\""));
        assert!(json.contains("\"example\":\"retry()\""));
    }

    #[test]
    fn json_render_recovery_no_example_omits_key() {
        let r = AgentResponse::new("body").with_recovery(RecoveryHint::new("retry", "wait and retry"));
        let json = r.render(OutputFormat::Json);
        assert!(!json.contains("\"example\""), "example key must be absent when None");
    }

    #[test]
    fn default_output_format_is_text() {
        assert_eq!(OutputFormat::default(), OutputFormat::Text);
    }

    #[test]
    fn json_render_capacity_scales_with_body_size() {
        let body = "x".repeat(10_000);
        let r = AgentResponse::new(body);
        let json = r.render(OutputFormat::Json);
        let slack = json.capacity() - json.len();
        assert!(
            slack < 500,
            "capacity {} vs len {} (slack {}) — capacity estimate should scale with body size, not balloon via repeated doubling from a fixed constant",
            json.capacity(),
            json.len(),
            slack
        );
    }
}
