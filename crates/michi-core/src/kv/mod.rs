use crate::hints::Hint;
use std::fmt::Write as _;
use std::time::Duration;

/// A single key-value pair for [`render_kv`].
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct KvItem {
    /// The field name.
    pub key: String,
    /// The field value.
    pub value: KvValue,
}

/// A value in a key-value item.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum KvValue {
    /// UTF-8 text value.
    Text(String),
    /// Signed integer.
    Int(i64),
    /// Floating-point number, rendered with the given number of decimal places.
    Float(f64, u8),
    /// Boolean renders as `true` or `false`.
    Bool(bool),
    /// Elapsed time, rendered as fractional seconds with one decimal.
    Duration(Duration),
    /// Absent value, renders as `—` (em dash).
    Missing,
}

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

    #[test]
    fn single_key_needs_no_padding() {
        let items = vec![KvItem { key: "id".into(), value: KvValue::Int(1) }];
        assert_eq!(render_kv(&items, None, &[]), "id: 1\n");
    }
}
