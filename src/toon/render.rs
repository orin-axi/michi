use std::fmt::Write as _;

use super::escape::escape_value;

/// A single cell value in a TOON row.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// UTF-8 string. Escaped if it contains commas, quotes, carriage returns, or newlines.
    Str(String),
    /// Signed integer.
    Int(i64),
    /// Floating-point number.
    Float(f64),
    /// Boolean renders as `true` or `false`.
    Bool(bool),
    /// Null renders as empty string (delimiter still emitted by the caller).
    Null,
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Str(s.to_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Str(s)
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Self::Float(f)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Self::Bool(b)
    }
}

impl From<Option<String>> for Value {
    fn from(s: Option<String>) -> Self {
        match s {
            Some(s) => Self::Str(s),
            None => Self::Null,
        }
    }
}

/// Render a TOON document string from its parts.
///
/// Pre-allocates output capacity based on row count × estimated row width.
pub(crate) fn render(
    type_name: &str,
    fields: &[String],
    rows: &[Vec<Value>],
    total_count: Option<usize>,
    hints: &[String],
) -> String {
    let row_count = rows.len();
    let field_count = fields.len();

    #[cfg(debug_assertions)]
    for row in rows {
        debug_assert!(
            row.len() == field_count,
            "row length {} does not match field count {field_count} (fields: {fields:?})",
            row.len()
        );
    }

    let capacity = 60 + row_count * (field_count * 12 + 10) + hints.len() * 60;
    let mut out = String::with_capacity(capacity);

    // type_name[count]{field,field,...}:
    out.push_str(type_name);
    out.push('[');
    out.push_str(&row_count.to_string());
    out.push_str("]{");
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(field);
    }
    out.push_str("}:\n");

    // rows
    for row in rows {
        out.push_str("  ");
        for (i, val) in row.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            match val {
                Value::Str(s) => out.push_str(&escape_value(s)),
                Value::Int(n) => {
                    let _ = write!(out, "{n}");
                }
                Value::Float(f) => {
                    let _ = write!(out, "{f}");
                }
                Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                Value::Null => {}
            }
        }
        out.push('\n');
    }

    // totalCount
    if let Some(total) = total_count {
        out.push_str("totalCount: ");
        out.push_str(&total.to_string());
        out.push('\n');
    }

    // help[N]: hints
    if !hints.is_empty() {
        out.push_str("help[");
        out.push_str(&hints.len().to_string());
        out.push_str("]:\n");
        for hint in hints {
            out.push_str("  ");
            out.push_str(hint);
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod value_conversion_tests {
    use super::Value;

    #[test]
    fn from_str_slice() {
        let v: Value = "hello".into();
        assert_eq!(v, Value::Str("hello".to_string()));
    }

    #[test]
    fn from_string() {
        let v: Value = "hello".to_string().into();
        assert_eq!(v, Value::Str("hello".to_string()));
    }

    #[test]
    fn from_i64() {
        let v: Value = 42i64.into();
        assert_eq!(v, Value::Int(42));
    }

    #[test]
    fn from_f64() {
        let v: Value = 1.5f64.into();
        assert_eq!(v, Value::Float(1.5));
    }

    #[test]
    fn from_bool() {
        let v: Value = true.into();
        assert_eq!(v, Value::Bool(true));
    }

    #[test]
    fn from_option_string_some() {
        let v: Value = Some("x".to_string()).into();
        assert_eq!(v, Value::Str("x".to_string()));
    }

    #[test]
    fn from_option_string_none() {
        let v: Value = None::<String>.into();
        assert_eq!(v, Value::Null);
    }
}

#[cfg(all(test, debug_assertions))]
mod row_length_tests {
    use super::{render, Value};

    #[test]
    #[should_panic(expected = "row length")]
    fn mismatched_row_length_panics_in_debug() {
        let fields = vec!["a".to_string(), "b".to_string()];
        let rows = vec![vec![Value::Int(1)]]; // 1 value, 2 fields declared
        render("t", &fields, &rows, None, &[]);
    }
}
