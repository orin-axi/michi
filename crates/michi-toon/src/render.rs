use compact_str::CompactString;
use std::fmt::Write as _;

use super::escape::{escape_value, sanitize_header_token, sanitize_hint, STRUCTURAL};

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

/// Proof that a `ToonOptions` value has passed structural validation.
///
/// The only function that produces a `ToonDocument` is [`ToonDocument::validate`],
/// declared in this same module alongside the private field, so no code inside
/// or outside this crate can obtain one without validation having returned `Ok`.
/// Borrows `&'a ToonOptions` immutably, so the validated options cannot be
/// mutated while the proof is live.
#[derive(Debug)]
pub struct ToonDocument<'a> {
    opts: &'a crate::ToonOptions,
}

impl<'a> ToonDocument<'a> {
    /// Validate `opts` and return a proof it satisfies every structural
    /// invariant [`ToonDocument::render`] depends on.
    ///
    /// # Errors
    ///
    /// Returns [`crate::ToonError::InvalidTypeName`] if `type_name` contains a
    /// structural character, [`crate::ToonError::InvalidFieldName`] if any
    /// field name contains a structural character, or
    /// [`crate::ToonError::RowLengthMismatch`] if any row's length differs
    /// from `fields.len()`.
    pub fn validate(opts: &'a crate::ToonOptions) -> Result<Self, crate::ToonError> {
        if opts.type_name.chars().any(|c| STRUCTURAL.contains(&c)) {
            return Err(crate::ToonError::InvalidTypeName { name: opts.type_name.clone() });
        }
        for field in &opts.fields {
            if field.chars().any(|c| STRUCTURAL.contains(&c)) {
                return Err(crate::ToonError::InvalidFieldName { name: field.clone() });
            }
        }
        for (i, row) in opts.rows.iter().enumerate() {
            if row.len() != opts.fields.len() {
                return Err(crate::ToonError::RowLengthMismatch {
                    row_index: i,
                    expected: opts.fields.len(),
                    actual: row.len(),
                });
            }
        }
        Ok(Self { opts })
    }

    /// Render `self` to a TOON document string. Infallible and total by
    /// construction: `self.opts`'s row arity was already proven equal to
    /// `self.opts.fields.len()` by `validate`, so this iterates rows and
    /// fields in lockstep instead of indexing by position, which could fail.
    #[must_use]
    pub fn render(&self) -> String {
        let opts = self.opts;
        let row_count = opts.rows.len();
        let field_count = opts.fields.len();
        let capacity = 60 + row_count * (field_count * 12 + 10) + opts.hints.len() * 60;
        let mut out = String::with_capacity(capacity);

        out.push_str(&sanitize_header_token(&opts.type_name));
        out.push('[');
        out.push_str(&row_count.to_string());
        out.push_str("]{");
        for (i, field) in opts.fields.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&sanitize_header_token(field));
        }
        out.push_str("}:\n");

        for row in &opts.rows {
            out.push_str("  ");
            for (i, cell) in row.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                match cell {
                    Value::Str(s) => {
                        if s.len() <= opts.max_cell_len {
                            out.push_str(&escape_value(s));
                        } else {
                            let truncated = michi_truncate::truncate_inline(s, opts.max_cell_len, "full=true");
                            out.push_str(&escape_value(&truncated));
                        }
                    }
                    Value::Int(n) => {
                        let _ = write!(out, "{n}");
                    }
                    Value::Float(f) => {
                        if f.is_nan() || f.is_infinite() {
                            let s = f.to_string();
                            let _ = write!(out, "\"{s}\"");
                        } else {
                            let _ = write!(out, "{f}");
                        }
                    }
                    Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                    Value::Null => {}
                }
            }
            out.push('\n');
        }

        if let Some(total) = opts.total_count {
            out.push_str("totalCount: ");
            out.push_str(&total.to_string());
            out.push('\n');
        }

        if !opts.hints.is_empty() {
            out.push_str("help[");
            out.push_str(&opts.hints.len().to_string());
            out.push_str("]:\n");
            for hint in &opts.hints {
                out.push_str("  ");
                out.push_str(&sanitize_hint(hint));
                out.push('\n');
            }
        }

        out
    }
}

#[allow(dead_code)]
fn _assert_render_signature(doc: &ToonDocument<'_>) -> String {
    doc.render()
}

#[cfg(test)]
mod tests {
    use super::Value;
    use compact_str::CompactString;

    fn render(
        type_name: &str,
        fields: &[String],
        rows: &[Vec<Value>],
        total_count: Option<usize>,
        hints: &[String],
        max_cell_len: usize,
    ) -> String {
        let opts = crate::ToonOptions {
            type_name: type_name.to_string(),
            fields: fields.to_vec(),
            rows: rows.to_vec(),
            total_count,
            hints: hints.to_vec(),
            max_cell_len,
        };
        crate::ToonDocument::validate(&opts).expect("test render() shim: options must validate").render()
    }

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
    fn float_nan_renders_as_quoted_string() {
        let out = render("t", &["v".to_string()], &[vec![Value::Float(f64::NAN)]], None, &[], 200);
        // NaN must not appear as bare token
        assert!(!out.contains(",NaN") && !out.contains("  NaN\n"), "got: {out}");
        // Must be quoted
        assert!(out.contains('"'), "NaN must be quoted, got: {out}");
    }

    #[test]
    fn float_inf_renders_as_quoted_string() {
        let out = render("t", &["v".to_string()], &[vec![Value::Float(f64::INFINITY)]], None, &[], 200);
        assert!(out.contains('"'), "inf must be quoted, got: {out}");
    }

    #[test]
    fn hint_with_newline_is_sanitized() {
        let out = render("t", &[], &[], None, &["line1\nline2".to_string()], 200);
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
    fn ac011_truncated_cell_content_is_exact() {
        let out = render("t", &["a".to_string()], &[vec![Value::from("a".repeat(500))]], None, &[], 200);
        let expected_prefix = "a".repeat(162);
        assert_eq!(out, format!("t[1]{{a}}:\n  {expected_prefix} (500 chars truncated — use full=true)\n"));
    }

    #[test]
    fn ac011_truncated_cell_strips_newlines_from_kept_prefix() {
        let content = "a\n".repeat(300);
        assert_eq!(content.chars().count(), 600);
        let out = render("t", &["a".to_string()], &[vec![Value::from(content)]], None, &[], 200);
        let expected_prefix = "a".repeat(81);
        assert_eq!(out, format!("t[1]{{a}}:\n  {expected_prefix} (600 chars truncated — use full=true)\n"));
        let cell = out.strip_prefix("t[1]{a}:\n  ").unwrap().strip_suffix('\n').unwrap();
        assert_eq!(cell.chars().count(), 119, "119, not the pre-escape 200, since newlines are stripped");
    }

    #[test]
    fn ac011b_max_cell_len_bounds_chars_not_bytes() {
        let content = "日".repeat(100);
        assert_eq!(content.chars().count(), 100);
        assert_eq!(content.len(), 300);
        let out = render("t", &["a".to_string()], &[vec![Value::from(content.clone())]], None, &[], 200);
        assert_eq!(
            out,
            format!("t[1]{{a}}:\n  {content}\n"),
            "a 300-byte/100-char cell must not truncate at max_cell_len=200"
        );
    }

    #[test]
    fn ac011c_max_cell_len_smaller_than_suffix_yields_suffix_prefix_only() {
        let out = render("t", &["a".to_string()], &[vec![Value::from("a".repeat(50))]], None, &[], 10);
        assert_eq!(out, "t[1]{a}:\n   (50 chars\n");
    }

    #[test]
    fn ac008_render_level_comma_cell_is_quoted_when_untruncated() {
        let out = render("t", &["a".to_string()], &[vec![Value::from("x,y")]], None, &[], 200);
        assert_eq!(out, "t[1]{a}:\n  \"x,y\"\n");
    }

    #[test]
    fn ac009_render_level_quote_cell_is_wrapped_and_escaped_when_untruncated() {
        let out = render("t", &["a".to_string()], &[vec![Value::from(r#"say "hi""#)]], None, &[], 200);
        assert_eq!(out, "t[1]{a}:\n  \"say \\\"hi\\\"\"\n");
    }

    #[test]
    fn ac008b_ac011d_truncated_cell_with_comma_in_kept_prefix_is_quoted() {
        let content = "a,".repeat(300);
        assert_eq!(content.chars().count(), 600);
        let out = render("t", &["a".to_string()], &[vec![Value::from(content)]], None, &[], 200);
        let expected_kept: String = "a,".repeat(300).chars().take(162).collect();
        let expected = format!("t[1]{{a}}:\n  \"{expected_kept} (600 chars truncated — use full=true)\"\n");
        assert_eq!(out, expected);
        let cell = out.strip_prefix("t[1]{a}:\n  ").unwrap().strip_suffix('\n').unwrap();
        assert_eq!(cell.chars().count(), 202);
        assert!(cell.starts_with('"') && cell.ends_with('"'));
    }

    #[test]
    fn ac012a_full_render_with_default_max_cell_len() {
        // Constructed via ToonOptions::new + render_toon (not super::render with a
        // literal 200) so a regression to ToonOptions::new's actual default is caught.
        let opts = crate::ToonOptions::new("t", vec!["a".to_string()], vec![vec![Value::from("a".repeat(500))]]);
        let out = crate::render_toon(&opts).unwrap();
        let expected_prefix = "a".repeat(162);
        assert_eq!(out, format!("t[1]{{a}}:\n  {expected_prefix} (500 chars truncated — use full=true)\n"));
    }

    #[test]
    fn ac012c_toon_options_new_defaults() {
        let opts = crate::ToonOptions::new("t", vec![], vec![]);
        assert_eq!(opts.total_count, None);
        assert!(opts.hints.is_empty());
        assert_eq!(opts.max_cell_len, 200);
    }

    #[test]
    fn ac012b_untruncated_cell_at_150_chars_near_boundary() {
        let content = "a".repeat(150);
        let opts = crate::ToonOptions::new("t", vec!["a".to_string()], vec![vec![Value::from(content.clone())]]);
        let out = crate::render_toon(&opts).unwrap();
        assert_eq!(out, format!("t[1]{{a}}:\n  {content}\n"));
        assert!(!out.contains("chars truncated"));
    }

    #[test]
    fn ac021_row_line_leading_space_in_cell_content_stacks_with_row_prefix() {
        let out = render("t", &["a".to_string()], &[vec![Value::from(" x")]], None, &[], 200);
        assert_eq!(out, "t[1]{a}:\n   x\n");
    }

    #[test]
    fn ac023_hint_line_leading_space_stacks_with_hint_prefix() {
        let out = render("t", &[], &[], None, &[" h".to_string()], 200);
        assert_eq!(out, "t[0]{}:\nhelp[1]:\n   h\n");
    }

    #[test]
    fn ac013_u64_and_usize_max_clamp_to_i64_max() {
        let v: Value = u64::MAX.into();
        assert_eq!(v, Value::Int(i64::MAX));
        let v: Value = usize::MAX.into();
        assert_eq!(v, Value::Int(i64::MAX));
        let out = render("t", &["n".to_string()], &[vec![u64::MAX.into()]], None, &[], 200);
        assert_eq!(out, "t[1]{n}:\n  9223372036854775807\n");
    }

    #[test]
    fn ac014_u64_and_usize_within_range_render_exact_digits() {
        let v: Value = 42u64.into();
        assert_eq!(v, Value::Int(42));
        let v: Value = 42usize.into();
        assert_eq!(v, Value::Int(42));
        let out = render("t", &["n".to_string()], &[vec![42u64.into()]], None, &[], 200);
        assert_eq!(out, "t[1]{n}:\n  42\n");
    }

    #[test]
    fn ac015_nan_renders_as_exact_quoted_text() {
        let out = render("t", &["v".to_string()], &[vec![Value::Float(f64::NAN)]], None, &[], 200);
        assert_eq!(out, "t[1]{v}:\n  \"NaN\"\n");
    }

    #[test]
    fn ac016_infinity_renders_as_exact_quoted_text() {
        let out = render("t", &["v".to_string()], &[vec![Value::Float(f64::INFINITY)]], None, &[], 200);
        assert_eq!(out, "t[1]{v}:\n  \"inf\"\n");
    }

    #[test]
    fn ac016b_neg_infinity_renders_as_exact_quoted_text() {
        let out = render("t", &["v".to_string()], &[vec![Value::Float(f64::NEG_INFINITY)]], None, &[], 200);
        assert_eq!(out, "t[1]{v}:\n  \"-inf\"\n");
    }

    #[test]
    fn ac023_empty_hints_produce_no_help_line() {
        let out = render("t", &[], &[], None, &[], 200);
        // A genuine `help[N]:` block line contains no `{` (the header, in
        // contrast, always contains `]{`); check for that exact form's absence.
        assert!(!out.lines().any(|l| l.starts_with("help[") && l.ends_with(':') && !l.contains('{')), "got: {out}");
    }

    #[test]
    fn ac023_type_name_help_does_not_falsely_trigger_the_negative_check() {
        // A header line starting with "help[" (from type_name "help") is not
        // itself of the form `help[N]:` as a standalone line -- it's the header,
        // which always contains "]{" and ends with "}:". This must not be
        // mistaken for a real help[N]: block.
        let out = render("help", &[], &[], None, &[], 200);
        assert_eq!(out, "help[0]{}:\n");
        assert!(!out.lines().any(|l| l.starts_with("help[") && l.ends_with(':') && !l.contains('{')), "got: {out}");
    }

    #[test]
    fn ac022_type_name_totalcount_does_not_falsely_trigger_the_negative_check() {
        let out = render("totalCount:", &[], &[], None, &[], 200);
        assert_eq!(out, "totalCount:[0]{}:\n");
        // A genuine `totalCount: N` line contains no `{`.
        assert!(!out.lines().any(|l| l.starts_with("totalCount: ") && !l.contains('{')), "got: {out}");
    }

    #[test]
    fn ac043_truncation_suffix_format_is_exact() {
        let out = render("t", &["a".to_string()], &[vec![Value::from("a".repeat(500))]], None, &[], 200);
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
        let out = render("t", &["a".to_string()], &[vec![Value::from("a".repeat(500))]], None, &[], max_cell_len);
        let cell = out.strip_prefix("t[1]{a}:\n  ").unwrap().strip_suffix('\n').unwrap();
        assert_eq!(cell.chars().count(), max_cell_len, "total cell content must be exactly max_cell_len chars");
        let kept: String = cell.chars().take_while(|c| *c == 'a').collect();
        assert_eq!(kept.chars().count(), expected_kept);

        // Saturating-at-0 case (per AC-011c): suffix char count > max_cell_len.
        let out10 = render("t", &["a".to_string()], &[vec![Value::from("a".repeat(50))]], None, &[], 10);
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

        let out_some = render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![some_via_option, Value::from("y")]],
            None,
            &[],
            200,
        );
        let out_str = render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![Value::from("x"), Value::from("y")]],
            None,
            &[],
            200,
        );
        assert_eq!(out_some, out_str);

        let out_none = render(
            "t",
            &["a".to_string(), "b".to_string()],
            &[vec![none_via_option, Value::from("y")]],
            None,
            &[],
            200,
        );
        let out_null =
            render("t", &["a".to_string(), "b".to_string()], &[vec![Value::Null, Value::from("y")]], None, &[], 200);
        assert_eq!(out_none, out_null);
    }
}

#[cfg(test)]
mod invariant_guard_tests {
    #[test]
    fn sole_constructor_for_toon_document() {
        let render_src = include_str!("render.rs");
        let lib_src = include_str!("lib.rs");
        let render_non_test = render_src.split("#[cfg(test)]").next().unwrap_or(render_src);
        let lib_non_test = lib_src.split("#[cfg(test)]").next().unwrap_or(lib_src);

        let known_ctor = render_non_test.matches("-> Result<Self, crate::ToonError>").count();
        assert_eq!(known_ctor, 1, "exactly one function must be ToonDocument's sole constructor; found {known_ctor}");
        let other_ctor_shapes = render_non_test.matches("-> ToonDocument").count();
        assert_eq!(
            other_ctor_shapes, 0,
            "no other function may return ToonDocument directly; found {other_ctor_shapes}"
        );

        let render_scan = render_non_test.to_lowercase().replace("unsafe_code", "");
        let lib_scan = lib_non_test.to_lowercase().replace("unsafe_code", "");
        for bad in ["unchecked", "unsafe", "raw", "assume"] {
            assert!(
                !render_scan.contains(bad) && !lib_scan.contains(bad),
                "escape-hatch name pattern {bad:?} must not appear in michi-toon src/"
            );
        }
    }

    #[test]
    fn toon_document_render_produces_expected_output() {
        let opts = crate::ToonOptions::new(
            "t",
            vec!["a".to_string(), "b".to_string()],
            vec![vec![crate::Value::from("x"), crate::Value::Int(1)]],
        )
        .total_count(Some(5))
        .hints(vec!["h".to_string()]);
        let doc = super::ToonDocument::validate(&opts).expect("valid options must validate");
        assert_eq!(doc.render(), "t[1]{a,b}:\n  x,1\ntotalCount: 5\nhelp[1]:\n  h\n");
    }

    #[test]
    fn no_loose_parameter_render_fn() {
        let src = include_str!("render.rs");
        assert!(
            !src.contains("fields: &[String],\n    rows: &[Vec<Value>],"),
            "the loose-parameter render(type_name, fields, rows, ...) function must not exist"
        );
    }

    #[test]
    fn no_profile_dependent_behavior() {
        let src = include_str!("render.rs");
        let non_test = src.split("#[cfg(test)]").next().unwrap_or(src);
        let debug_assert_needle = ["debug", "_", "assert", "!"].concat();
        let debug_assert_eq_needle = ["debug", "_", "assert", "_", "eq", "!"].concat();
        let debug_assert_ne_needle = ["debug", "_", "assert", "_", "ne", "!"].concat();
        let cfg_debug_needle = ["cfg", "(", "debug_assertions", ")"].concat();
        for token in [&debug_assert_needle, &debug_assert_eq_needle, &debug_assert_ne_needle, &cfg_debug_needle] {
            assert!(!non_test.contains(token.as_str()), "found profile-dependent token {token:?} in render.rs");
        }
    }

    #[test]
    fn render_has_no_missing_cell_path() {
        let src = include_str!("render.rs");
        let non_test = src.split("#[cfg(test)]").next().unwrap_or(src);
        let row_get_needle = ["row", ".", "get", "("].concat();
        assert!(
            !non_test.contains(&row_get_needle),
            "ToonDocument::render must iterate cells directly, not via row.get(i)"
        );
    }
}
