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

    #[test]
    #[cfg_attr(not(debug_assertions), ignore = "AC-006a only fires when debug_assertions is enabled")]
    #[should_panic(expected = "row length 1 does not match field count 2")]
    fn ac006a_short_row_panics_in_debug() {
        super::render("t", &["a".to_string(), "b".to_string()], &[vec![Value::from("x")]], None, &[], 200);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn ac006b_short_row_renders_empty_cell_in_release() {
        let out = super::render("t", &["a".to_string(), "b".to_string()], &[vec![Value::from("x")]], None, &[], 200);
        assert_eq!(out, "t[1]{a,b}:\n  x,\n");
    }

    #[test]
    #[cfg_attr(not(debug_assertions), ignore = "AC-007a only fires when debug_assertions is enabled")]
    #[should_panic(expected = "row length 2 does not match field count 1")]
    fn ac007a_long_row_panics_in_debug() {
        super::render("t", &["a".to_string()], &[vec![Value::from("x"), Value::from("y")]], None, &[], 200);
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn ac007b_long_row_drops_extra_cells_in_release() {
        let out = super::render("t", &["a".to_string()], &[vec![Value::from("x"), Value::from("y")]], None, &[], 200);
        assert_eq!(out, "t[1]{a}:\n  x\n");
    }

    #[test]
    #[cfg(not(debug_assertions))]
    fn ac018_null_and_missing_cell_render_identically_in_release() {
        let null_row = super::render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![Value::from("x"), Value::Null]],
            None,
            &[],
            200,
        );
        let missing_row =
            super::render("t", &["a".to_string(), "b".to_string()], &[vec![Value::from("x")]], None, &[], 200);
        assert_eq!(null_row, missing_row);
    }

    #[test]
    fn ac011_truncated_cell_content_is_exact() {
        let out = super::render("t", &["a".to_string()], &[vec![Value::from("a".repeat(500))]], None, &[], 200);
        let expected_prefix = "a".repeat(162);
        assert_eq!(out, format!("t[1]{{a}}:\n  {expected_prefix} (500 chars truncated — use full=true)\n"));
    }

    #[test]
    fn ac011b_max_cell_len_bounds_chars_not_bytes() {
        let content = "日".repeat(100);
        assert_eq!(content.chars().count(), 100);
        assert_eq!(content.len(), 300);
        let out = super::render("t", &["a".to_string()], &[vec![Value::from(content.clone())]], None, &[], 200);
        assert_eq!(
            out,
            format!("t[1]{{a}}:\n  {content}\n"),
            "a 300-byte/100-char cell must not truncate at max_cell_len=200"
        );
    }

    #[test]
    fn ac011c_max_cell_len_smaller_than_suffix_yields_suffix_prefix_only() {
        let out = super::render("t", &["a".to_string()], &[vec![Value::from("a".repeat(50))]], None, &[], 10);
        assert_eq!(out, "t[1]{a}:\n   (50 chars\n");
    }

    #[test]
    fn ac008_render_level_comma_cell_is_quoted_when_untruncated() {
        let out = super::render("t", &["a".to_string()], &[vec![Value::from("x,y")]], None, &[], 200);
        assert_eq!(out, "t[1]{a}:\n  \"x,y\"\n");
    }

    #[test]
    fn ac009_render_level_quote_cell_is_wrapped_and_escaped_when_untruncated() {
        let out = super::render("t", &["a".to_string()], &[vec![Value::from(r#"say "hi""#)]], None, &[], 200);
        assert_eq!(out, "t[1]{a}:\n  \"say \\\"hi\\\"\"\n");
    }

    #[test]
    fn ac008b_ac011d_truncated_cell_with_comma_in_kept_prefix_is_quoted() {
        let content = "a,".repeat(300);
        assert_eq!(content.chars().count(), 600);
        let out = super::render("t", &["a".to_string()], &[vec![Value::from(content)]], None, &[], 200);
        let expected_kept: String = "a,".repeat(300).chars().take(162).collect();
        let expected = format!("t[1]{{a}}:\n  \"{expected_kept} (600 chars truncated — use full=true)\"\n");
        assert_eq!(out, expected);
        let cell = out.strip_prefix("t[1]{a}:\n  ").unwrap().strip_suffix('\n').unwrap();
        assert_eq!(cell.chars().count(), 202);
        assert!(cell.starts_with('"') && cell.ends_with('"'));
    }

    #[test]
    fn ac005_structural_type_name_sanitizes_without_panicking_when_rows_well_formed() {
        let out = super::render(
            "ty[pe",
            &["a".to_string(), "b".to_string()],
            &[vec![Value::from("x"), Value::from("y")]],
            None,
            &[],
            200,
        );
        assert_eq!(out, "ty_pe[1]{a,b}:\n  x,y\n");
    }

    #[test]
    fn ac020_structural_type_name_is_sanitized_not_preserved_in_header() {
        let out = super::render("ty[pe", &["a".to_string()], &[], None, &[], 200);
        assert_eq!(out, "ty_pe[0]{a}:\n");
    }

    #[test]
    fn ac012a_full_render_with_default_max_cell_len() {
        let opts_row = vec![Value::from("a".repeat(500))];
        let out = super::render("t", &["a".to_string()], &[opts_row], None, &[], 200);
        let expected_prefix = "a".repeat(162);
        assert_eq!(out, format!("t[1]{{a}}:\n  {expected_prefix} (500 chars truncated — use full=true)\n"));
    }

    #[test]
    fn ac012b_untruncated_cell_at_150_chars_near_boundary() {
        let content = "a".repeat(150);
        let out = super::render("t", &["a".to_string()], &[vec![Value::from(content.clone())]], None, &[], 200);
        assert_eq!(out, format!("t[1]{{a}}:\n  {content}\n"));
        assert!(!out.contains("chars truncated"));
    }

    #[test]
    fn ac013_u64_and_usize_max_clamp_to_i64_max() {
        let v: Value = u64::MAX.into();
        assert_eq!(v, Value::Int(i64::MAX));
        let v: Value = usize::MAX.into();
        assert_eq!(v, Value::Int(i64::MAX));
        let out = super::render("t", &["n".to_string()], &[vec![u64::MAX.into()]], None, &[], 200);
        assert_eq!(out, "t[1]{n}:\n  9223372036854775807\n");
    }

    #[test]
    fn ac014_u64_and_usize_within_range_render_exact_digits() {
        let v: Value = 42u64.into();
        assert_eq!(v, Value::Int(42));
        let v: Value = 42usize.into();
        assert_eq!(v, Value::Int(42));
        let out = super::render("t", &["n".to_string()], &[vec![42u64.into()]], None, &[], 200);
        assert_eq!(out, "t[1]{n}:\n  42\n");
    }

    #[test]
    fn ac015_nan_renders_as_exact_quoted_text() {
        let out = super::render("t", &["v".to_string()], &[vec![Value::Float(f64::NAN)]], None, &[], 200);
        assert_eq!(out, "t[1]{v}:\n  \"NaN\"\n");
    }

    #[test]
    fn ac016_infinity_renders_as_exact_quoted_text() {
        let out = super::render("t", &["v".to_string()], &[vec![Value::Float(f64::INFINITY)]], None, &[], 200);
        assert_eq!(out, "t[1]{v}:\n  \"inf\"\n");
    }

    #[test]
    fn ac016b_neg_infinity_renders_as_exact_quoted_text() {
        let out = super::render("t", &["v".to_string()], &[vec![Value::Float(f64::NEG_INFINITY)]], None, &[], 200);
        assert_eq!(out, "t[1]{v}:\n  \"-inf\"\n");
    }

    #[test]
    fn ac023_empty_hints_produce_no_help_line() {
        let out = super::render("t", &[], &[], None, &[], 200);
        assert!(!out.lines().any(|l| l.starts_with("help[")), "got: {out}");
    }

    #[test]
    fn ac043_truncation_suffix_format_is_exact() {
        let out = super::render("t", &["a".to_string()], &[vec![Value::from("a".repeat(500))]], None, &[], 200);
        assert!(out.ends_with(" (500 chars truncated — use full=true)\n"), "got: {out:?}");
        assert!(out.contains('\u{2014}'), "must use EM DASH U+2014, got: {out:?}");
        assert!(!out.contains('-'), "must not contain a hyphen anywhere in the suffix, got: {out:?}");
    }

    #[test]
    fn ac044_kept_prefix_length_equals_max_cell_len_minus_suffix_chars() {
        let suffix = " (500 chars truncated — use full=true)";
        let suffix_chars = suffix.chars().count();
        let max_cell_len = 200usize;
        let expected_kept = max_cell_len - suffix_chars;
        let out =
            super::render("t", &["a".to_string()], &[vec![Value::from("a".repeat(500))]], None, &[], max_cell_len);
        let cell = out.strip_prefix("t[1]{a}:\n  ").unwrap().strip_suffix('\n').unwrap();
        assert_eq!(cell.chars().count(), max_cell_len, "total cell content must be exactly max_cell_len chars");
        let kept: String = cell.chars().take_while(|c| *c == 'a').collect();
        assert_eq!(kept.chars().count(), expected_kept);

        // Saturating-at-0 case (per AC-011c): suffix char count > max_cell_len.
        let out10 = super::render("t", &["a".to_string()], &[vec![Value::from("a".repeat(50))]], None, &[], 10);
        let cell10 = out10.strip_prefix("t[1]{a}:\n  ").unwrap().strip_suffix('\n').unwrap();
        assert!(!cell10.starts_with('a'), "kept prefix must saturate to 0 chars, got: {cell10:?}");
        assert!(cell10.chars().count() <= 10);
    }

    #[test]
    fn ac036_from_option_string_some_matches_str_none_matches_null() {
        let some_via_option: Value = Some("x".to_string()).into();
        assert_eq!(some_via_option, Value::from("x"));
        let none_via_option: Value = None::<String>.into();
        assert_eq!(none_via_option, Value::Null);

        let out_some = super::render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![some_via_option, Value::from("y")]],
            None,
            &[],
            200,
        );
        let out_str = super::render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![Value::from("x"), Value::from("y")]],
            None,
            &[],
            200,
        );
        assert_eq!(out_some, out_str);

        let out_none = super::render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![none_via_option, Value::from("y")]],
            None,
            &[],
            200,
        );
        let out_null = super::render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![Value::Null, Value::from("y")]],
            None,
            &[],
            200,
        );
        assert_eq!(out_none, out_null);
    }
}
