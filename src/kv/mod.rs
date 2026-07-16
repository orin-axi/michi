use crate::hints::Hint;
use std::fmt::Write as _;
use std::time::Duration;

/// A single key-value pair for [`render_kv`].
#[derive(Debug, Clone, PartialEq)]
pub struct KvItem {
    /// The field name.
    pub key: String,
    /// The field value.
    pub value: KvValue,
}

/// A value in a key-value item.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum KvValue {
    /// UTF-8 text value.
    Text(String),
    /// Signed integer.
    Int(i64),
    /// Floating-point number, rendered with the given number of decimal places.
    Float(f64, u8),
    /// Boolean renders as `true` or `false`.
    Bool(bool),
    /// Elapsed time, rendered as fractional seconds with one decimal (e.g. `4.2s`).
    Duration(Duration),
    /// Absent value, renders as `—` (em dash) rather than an empty string —
    /// distinguishes "no value" from "empty string value".
    Missing,
}

/// Append a single [`KvValue`]'s plain-text rendering to `out` (no key, no
/// trailing newline). Shared with [`crate::status`], which reuses this to
/// keep the `KvValue` match in one place.
pub(crate) fn push_kv_value(out: &mut String, value: &KvValue) {
    match value {
        KvValue::Text(s) => out.push_str(s),
        KvValue::Int(n) => {
            let _ = write!(out, "{n}");
        }
        KvValue::Float(f, decimals) => {
            let _ = write!(out, "{f:.*}", *decimals as usize);
        }
        KvValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        KvValue::Duration(d) => {
            let _ = write!(out, "{:.1}s", d.as_secs_f64());
        }
        KvValue::Missing => out.push('—'),
    }
}

/// Append a JSON-encoded string (with surrounding quotes and escape sequences)
/// to `out`. Escapes `"`, `\`, `\n`, `\r`, `\t`, and all other control
/// characters (U+0000–U+001F) as `\u00XX` per RFC 8259. Lives here (rather
/// than in `response.rs`, its only other caller) because [`kv_value_to_json`]
/// needs it and `kv` sits below `response` in the module dependency order.
pub(crate) fn json_escape_str(out: &mut String, s: &str) {
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
                out.push(char::from_digit(code >> 4, 16).unwrap_or('0'));
                out.push(char::from_digit(code & 0xf, 16).unwrap_or('0'));
            }
            other => out.push(other),
        }
    }
    out.push('"');
}

/// Append a single [`KvValue`]'s JSON representation to `out`, using native
/// JSON types — `Int`/`Float`/`Bool` render as unquoted JSON literals and
/// `Missing` renders as `null` — rather than the stringified form
/// [`push_kv_value`] produces for plain-text output. `Duration` has no native
/// JSON scalar, so it renders as a JSON string using the same `"4.2s"`
/// formatting as the plain-text path.
///
/// Used by JSON-rendering call sites (e.g.
/// [`crate::response::AgentResponse::render_json`]'s recovery params) so a
/// downstream JSON consumer (a TypeScript/MCP client, say) sees typed values
/// instead of every value flattened to a string.
pub(crate) fn kv_value_to_json(out: &mut String, value: &KvValue) {
    match value {
        KvValue::Text(s) => json_escape_str(out, s),
        KvValue::Int(n) => {
            let _ = write!(out, "{n}");
        }
        KvValue::Float(f, decimals) => {
            let _ = write!(out, "{f:.*}", *decimals as usize);
        }
        KvValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        KvValue::Duration(d) => {
            let mut secs = String::new();
            let _ = write!(secs, "{:.1}s", d.as_secs_f64());
            json_escape_str(out, &secs);
        }
        KvValue::Missing => out.push_str("null"),
    }
}

/// Render a list of key-value pairs as a column-aligned multi-line block.
///
/// Keys are left-padded with spaces so every `:` lines up on the longest key.
/// Appends a `totalCount: N` line when `total_count` is `Some`, and a
/// `help[N]:` block when `hints` is non-empty.
///
/// Preferred for single items and small metadata blocks (up to ~5 fields).
/// For lists of 5+ items, prefer [`crate::toon::render_toon`].
///
/// Returns an empty string when `items` is empty.
#[must_use]
pub fn render_kv(items: &[KvItem], total_count: Option<usize>, hints: &[Hint]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let max_key_len = items.iter().map(|i| i.key.chars().count()).max().unwrap_or(0);
    let capacity = items.len() * (max_key_len + 24) + hints.len() * 50 + 20;
    let mut out = String::with_capacity(capacity);
    for item in items {
        out.push_str(&item.key);
        out.push(':');
        let pad = max_key_len - item.key.chars().count() + 1;
        for _ in 0..pad {
            out.push(' ');
        }
        push_kv_value(&mut out, &item.value);
        out.push('\n');
    }
    if let Some(total) = total_count {
        out.push_str("totalCount: ");
        let _ = write!(out, "{total}");
        out.push('\n');
    }
    crate::hints::append_hints(&mut out, hints);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hints::Hint;
    use std::time::Duration;

    #[test]
    fn columns_are_aligned_to_longest_key() {
        let items = vec![
            KvItem { key: "name".into(), value: KvValue::Text("Button".into()) },
            KvItem { key: "description".into(), value: KvValue::Text("A button".into()) },
        ];
        let out = render_kv(&items, None, &[]);
        // "description" (11 chars) is the longest key; "name" must be padded to match.
        // pad = max_key_len - key_len + 1: name gets 8 spaces, description gets 1.
        assert_eq!(out, "name:        Button\ndescription: A button\n");
    }

    #[test]
    fn total_count_appends_line() {
        let items = vec![KvItem { key: "id".into(), value: KvValue::Int(1) }];
        let out = render_kv(&items, Some(5), &[]);
        assert!(out.contains("totalCount: 5\n"));
    }

    #[test]
    fn hints_append_help_block() {
        let items = vec![KvItem { key: "id".into(), value: KvValue::Int(1) }];
        let out = render_kv(&items, None, &[Hint::new("do this")]);
        assert!(out.contains("help[1]:\n  do this\n"));
    }

    #[test]
    fn missing_renders_as_em_dash() {
        let items = vec![KvItem { key: "value".into(), value: KvValue::Missing }];
        assert_eq!(render_kv(&items, None, &[]), "value: —\n");
    }

    #[test]
    fn float_respects_decimal_places() {
        let items = vec![KvItem { key: "ratio".into(), value: KvValue::Float(1.0 / 3.0, 2) }];
        assert!(render_kv(&items, None, &[]).contains("ratio: 0.33"));
    }

    #[test]
    fn duration_renders_as_seconds_with_one_decimal() {
        let items = vec![KvItem { key: "elapsed".into(), value: KvValue::Duration(Duration::from_millis(4200)) }];
        assert!(render_kv(&items, None, &[]).contains("elapsed: 4.2s"));
    }

    #[test]
    fn text_and_bool_render_as_before() {
        let items = vec![
            KvItem { key: "status".into(), value: KvValue::Text("open".into()) },
            KvItem { key: "active".into(), value: KvValue::Bool(true) },
        ];
        let out = render_kv(&items, None, &[]);
        assert!(out.contains("status: open\n"));
        assert!(out.contains("active: true\n"));
    }

    #[test]
    fn empty_items_returns_empty_string() {
        assert_eq!(render_kv(&[], None, &[]), "");
    }

    #[test]
    fn single_key_needs_no_padding() {
        let items = vec![KvItem { key: "id".into(), value: KvValue::Int(1) }];
        assert_eq!(render_kv(&items, None, &[]), "id: 1\n");
    }

    #[test]
    fn kv_value_to_json_renders_int_float_bool_as_native_literals() {
        let mut out = String::new();
        kv_value_to_json(&mut out, &KvValue::Int(30));
        assert_eq!(out, "30");

        let mut out = String::new();
        kv_value_to_json(&mut out, &KvValue::Float(1.0 / 3.0, 2));
        assert_eq!(out, "0.33");

        let mut out = String::new();
        kv_value_to_json(&mut out, &KvValue::Bool(true));
        assert_eq!(out, "true");
    }

    #[test]
    fn kv_value_to_json_renders_text_as_quoted_string() {
        let mut out = String::new();
        kv_value_to_json(&mut out, &KvValue::Text("alice".into()));
        assert_eq!(out, "\"alice\"");
    }

    #[test]
    fn kv_value_to_json_renders_missing_as_null() {
        let mut out = String::new();
        kv_value_to_json(&mut out, &KvValue::Missing);
        assert_eq!(out, "null");
    }

    #[test]
    fn kv_value_to_json_escapes_text_control_characters() {
        let mut out = String::new();
        kv_value_to_json(&mut out, &KvValue::Text("line1\nline2\t\"quoted\"".into()));
        assert_eq!(out, "\"line1\\nline2\\t\\\"quoted\\\"\"");
    }

    #[test]
    #[cfg(feature = "serde")]
    fn kv_value_serializes_and_deserializes() {
        let v = KvValue::Text("hello".to_string());
        let json = serde_json::to_string(&v).expect("serializes");
        let back: KvValue = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(v, back);
    }
}
