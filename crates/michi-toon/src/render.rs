use compact_str::CompactString;
use std::fmt::Write as _;

use super::escape::{escape_value, sanitize_header_token, sanitize_hint};

/// A single cell value in a TOON row.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub enum Value {
    /// UTF-8 string (stack-inlined for strings <= 24 bytes via `CompactString`).
    #[cfg_attr(feature = "schemars", schemars(with = "String"))]
    Str(CompactString),
    /// Signed integer.
    Int(i64),
    /// Floating-point number.
    Float(f64),
    /// Boolean renders as `true` or `false`.
    Bool(bool),
    /// Null renders as empty string.
    Null,
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Self::Str(CompactString::new(s))
    }
}

impl From<&String> for Value {
    fn from(s: &String) -> Self {
        Self::Str(CompactString::new(s))
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Self::Str(CompactString::new(s))
    }
}

impl From<CompactString> for Value {
    fn from(s: CompactString) -> Self {
        Self::Str(s)
    }
}

impl From<std::borrow::Cow<'_, str>> for Value {
    fn from(s: std::borrow::Cow<'_, str>) -> Self {
        Self::Str(CompactString::new(s))
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Self::Int(i64::from(n))
    }
}

impl From<u32> for Value {
    fn from(n: u32) -> Self {
        Self::Int(i64::from(n))
    }
}

impl From<i64> for Value {
    fn from(n: i64) -> Self {
        Self::Int(n)
    }
}

impl From<u64> for Value {
    fn from(n: u64) -> Self {
        // Values > i64::MAX are clamped to i64::MAX — u64 cannot be represented
        // losslessly in TOON's Int type. Callers with hashes or large counters
        // that exceed i64::MAX should convert to a string value instead.
        #[allow(clippy::cast_possible_wrap)]
        Self::Int(n.try_into().unwrap_or(i64::MAX))
    }
}

impl From<usize> for Value {
    fn from(n: usize) -> Self {
        #[allow(clippy::cast_possible_wrap)]
        Self::Int(n.try_into().unwrap_or(i64::MAX))
    }
}

impl From<f32> for Value {
    fn from(f: f32) -> Self {
        Self::Float(f64::from(f))
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
            Some(s) => Self::Str(CompactString::new(s)),
            None => Self::Null,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Str(s) => f.write_str(s),
            Self::Int(n) => write!(f, "{n}"),
            Self::Float(fl) => write!(f, "{fl}"),
            Self::Bool(b) => f.write_str(if *b { "true" } else { "false" }),
            Self::Null => Ok(()),
        }
    }
}

/// Render a TOON document string from its parts.
pub(crate) fn render(
    type_name: &str,
    fields: &[String],
    rows: &[Vec<Value>],
    total_count: Option<usize>,
    hints: &[String],
    max_cell_len: usize,
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
    out.push_str(&sanitize_header_token(type_name));
    out.push('[');
    out.push_str(&row_count.to_string());
    out.push_str("]{");
    for (i, field) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&sanitize_header_token(field));
    }
    out.push_str("}:\n");

    // rows
    for row in rows {
        out.push_str("  ");
        for i in 0..field_count {
            if i > 0 {
                out.push(',');
            }
            match row.get(i) {
                Some(Value::Str(s)) => {
                    if s.len() <= max_cell_len {
                        out.push_str(&escape_value(s));
                    } else {
                        let truncated = michi_truncate::truncate_inline(s, max_cell_len, "full=true");
                        out.push_str(&escape_value(&truncated));
                    }
                }
                Some(Value::Int(n)) => {
                    let _ = write!(out, "{n}");
                }
                Some(Value::Float(f)) => {
                    if f.is_nan() || f.is_infinite() {
                        // NaN and Inf are not valid TOON scalars — render as quoted strings
                        let s = f.to_string();
                        let _ = write!(out, "\"{s}\"");
                    } else {
                        let _ = write!(out, "{f}");
                    }
                }
                Some(Value::Bool(b)) => out.push_str(if *b { "true" } else { "false" }),
                Some(Value::Null) | None => {} // Null or missing cell → empty
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
            out.push_str(&sanitize_hint(hint));
            out.push('\n');
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::Value;
    use compact_str::CompactString;

    #[test]
    fn from_str_slice() {
        let v: Value = "hello".into();
        assert_eq!(v, Value::Str(CompactString::new("hello")));
    }

    #[test]
    fn from_i64() {
        let v: Value = 42i64.into();
        assert_eq!(v, Value::Int(42));
    }

    #[test]
    fn type_name_with_newline_is_sanitized() {
        let out = super::render("foo\nbar", &[], &[], None, &[], 200);
        assert!(out.starts_with("foo_bar[0]{}:"), "got: {out}");
    }

    #[test]
    fn field_with_comma_is_sanitized() {
        let out = super::render("t", &["a,b".to_string()], &[vec![Value::from("v")]], None, &[], 200);
        assert!(out.contains("{a_b}"), "got: {out}");
    }

    #[test]
    fn float_nan_renders_as_quoted_string() {
        let out = super::render("t", &["v".to_string()], &[vec![Value::Float(f64::NAN)]], None, &[], 200);
        // NaN must not appear as bare token
        assert!(!out.contains(",NaN") && !out.contains("  NaN\n"), "got: {out}");
        // Must be quoted
        assert!(out.contains('"'), "NaN must be quoted, got: {out}");
    }

    #[test]
    fn float_inf_renders_as_quoted_string() {
        let out = super::render("t", &["v".to_string()], &[vec![Value::Float(f64::INFINITY)]], None, &[], 200);
        assert!(out.contains('"'), "inf must be quoted, got: {out}");
    }

    #[test]
    fn hint_with_newline_is_sanitized() {
        let out = super::render("t", &[], &[], None, &["line1\nline2".to_string()], 200);
        // hint must not inject a raw newline into the output
        let hint_section: String =
            out.lines().skip_while(|l| !l.starts_with("help[")).skip(1).collect::<Vec<_>>().join("\n");
        assert!(
            !hint_section.contains('\n') || hint_section.lines().count() <= 1,
            "hint newline must be replaced, hint section: {hint_section:?}"
        );
        assert!(out.contains("line1_line2"), "got: {out}");
    }
}
