use std::fmt::Write as _;

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
pub enum KvValue {
    /// UTF-8 string value.
    Str(String),
    /// Signed integer.
    Int(i64),
    /// Floating-point number.
    Float(f64),
    /// Boolean renders as `true` or `false`.
    Bool(bool),
    /// Null renders as empty string after the colon.
    Null,
}

/// Render a list of key-value pairs as a multi-line `key: value` block.
///
/// Preferred for single items and small metadata blocks (up to ~5 fields).
/// For lists of 5+ items, prefer [`crate::toon::render_toon`].
///
/// Returns an empty string when `items` is empty.
#[must_use]
pub fn render_kv(items: &[KvItem]) -> String {
    if items.is_empty() {
        return String::new();
    }
    let capacity = items.len() * 20;
    let mut out = String::with_capacity(capacity);
    for item in items {
        out.push_str(&item.key);
        out.push_str(": ");
        match &item.value {
            KvValue::Str(s) => out.push_str(s),
            KvValue::Int(n) => {
                let _ = write!(out, "{n}");
            }
            KvValue::Float(f) => {
                let _ = write!(out, "{f}");
            }
            KvValue::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            KvValue::Null => {}
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_kv() {
        let items = vec![
            KvItem { key: "id".into(), value: KvValue::Str("abc-123".into()) },
            KvItem { key: "status".into(), value: KvValue::Str("open".into()) },
            KvItem { key: "count".into(), value: KvValue::Int(42) },
        ];
        assert_eq!(render_kv(&items), "id: abc-123\nstatus: open\ncount: 42\n");
    }

    #[test]
    fn renders_null_as_empty() {
        let items = vec![KvItem { key: "value".into(), value: KvValue::Null }];
        assert_eq!(render_kv(&items), "value: \n");
    }

    #[test]
    fn renders_bool() {
        let items = vec![
            KvItem { key: "active".into(), value: KvValue::Bool(true) },
            KvItem { key: "deleted".into(), value: KvValue::Bool(false) },
        ];
        assert_eq!(render_kv(&items), "active: true\ndeleted: false\n");
    }

    #[test]
    fn empty_items_returns_empty_string() {
        assert_eq!(render_kv(&[]), "");
    }

    #[test]
    fn renders_float() {
        let items = vec![KvItem { key: "ratio".into(), value: KvValue::Float(0.5) }];
        assert!(render_kv(&items).contains("ratio: 0.5"));
    }

    #[test]
    fn renders_numeric_formatting_exact() {
        let items = vec![
            KvItem { key: "neg".into(), value: KvValue::Int(-42) },
            KvItem { key: "zero".into(), value: KvValue::Int(0) },
            KvItem { key: "big".into(), value: KvValue::Int(i64::MAX) },
            KvItem { key: "neg_float".into(), value: KvValue::Float(-1.25) },
            KvItem { key: "whole_float".into(), value: KvValue::Float(2.0) },
        ];
        assert_eq!(
            render_kv(&items),
            "neg: -42\nzero: 0\nbig: 9223372036854775807\nneg_float: -1.25\nwhole_float: 2\n"
        );
    }
}
