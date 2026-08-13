/// Escape a scalar value for TOON row output.
pub fn escape_value(v: &str) -> std::borrow::Cow<'_, str> {
    if v.is_empty() {
        return std::borrow::Cow::Borrowed(v);
    }

    let mut needs_quote = false;
    let mut quote_count = 0usize;
    let mut stripped_count = 0usize;
    for ch in v.chars() {
        match ch {
            ',' => needs_quote = true,
            '"' => {
                needs_quote = true;
                quote_count += 1;
            }
            '\n' | '\r' => stripped_count += 1,
            _ => {}
        }
    }

    if !needs_quote && stripped_count == 0 {
        return std::borrow::Cow::Borrowed(v);
    }

    let capacity = v.len() - stripped_count + quote_count + if needs_quote { 2 } else { 0 };
    let mut out = String::with_capacity(capacity);
    if needs_quote {
        out.push('"');
    }
    for ch in v.chars() {
        match ch {
            '\n' | '\r' => {}
            '"' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    if needs_quote {
        out.push('"');
    }
    std::borrow::Cow::Owned(out)
}

/// Escape a value for TOON row output and unconditionally wrap it in quotes.
pub fn escape_value_quoted(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for ch in v.chars() {
        match ch {
            '\n' | '\r' => {}
            '"' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Characters that are structurally significant in TOON header positions and
/// must not appear verbatim in type names, field names, or hints.
///
/// Shared with [`ToonOptions::validate()`] to keep the two in sync.
pub(crate) const STRUCTURAL: &[char] = &['[', ']', '{', '}', ',', '\n', '\r'];

/// Sanitize a TOON header token (type_name or field name) for safe embedding.
///
/// Replaces `\n`, `\r`, and structural characters (`[`, `]`, `{`, `}`, `,`)
/// with `_`. Header positions have no escaping syntax in the TOON grammar —
/// replacement is the only safe option that keeps output parseable.
#[must_use]
pub(crate) fn sanitize_header_token(s: &str) -> std::borrow::Cow<'_, str> {
    if s.chars().any(|c| STRUCTURAL.contains(&c)) {
        std::borrow::Cow::Owned(s.chars().map(|c| if STRUCTURAL.contains(&c) { '_' } else { c }).collect())
    } else {
        std::borrow::Cow::Borrowed(s)
    }
}

/// Sanitize a TOON hint string. Hint positions have the same structural
/// constraint as header tokens.
pub(crate) use sanitize_header_token as sanitize_hint;

#[cfg(test)]
mod sanitize_tests {
    use super::*;

    #[test]
    fn plain_token_is_borrowed() {
        let result = sanitize_header_token("file_path");
        assert!(matches!(result, std::borrow::Cow::Borrowed(_)));
        assert_eq!(result, "file_path");
    }

    #[test]
    fn newline_in_type_name_is_replaced() {
        assert_eq!(sanitize_header_token("foo\nbar"), "foo_bar");
    }

    #[test]
    fn carriage_return_is_replaced() {
        assert_eq!(sanitize_header_token("foo\rbar"), "foo_bar");
    }

    #[test]
    fn structural_chars_are_replaced() {
        assert_eq!(sanitize_header_token("a[b]c"), "a_b_c");
        assert_eq!(sanitize_header_token("a{b}c"), "a_b_c");
        assert_eq!(sanitize_header_token("a,b"), "a_b");
    }

    #[test]
    fn multiple_structural_chars() {
        assert_eq!(sanitize_header_token("foo{bar},baz\n"), "foo_bar__baz_");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_value_is_not_escaped() {
        assert_eq!(escape_value("hello"), "hello");
    }

    #[test]
    fn value_with_comma_is_quoted() {
        assert_eq!(escape_value("a,b"), r#""a,b""#);
    }

    #[test]
    fn value_with_quote_escapes_it() {
        assert_eq!(escape_value(r#"say "hi""#), r#""say \"hi\"""#);
    }

    #[test]
    fn empty_value_is_unchanged() {
        assert_eq!(escape_value(""), "");
    }

    #[test]
    fn ac031_escape_value_quoted_always_wraps_in_quotes() {
        for s in ["", "plain", "a,b", "line1\nline2"] {
            let out = escape_value_quoted(s);
            assert!(out.starts_with('"') && out.ends_with('"'), "input {s:?} -> {out:?}");
        }
    }

    #[test]
    fn ac033_escape_value_quoted_escapes_embedded_quotes() {
        assert_eq!(escape_value_quoted(r#"say "hi""#), r#""say \"hi\"""#);
    }

    #[test]
    fn ac034_escape_value_quoted_strips_newlines_and_carriage_returns() {
        let out = escape_value_quoted("a\nb\rc");
        assert!(!out.contains('\n') && !out.contains('\r'), "got: {out:?}");
        assert_eq!(out, "\"abc\"");
    }

    #[test]
    fn ac042_escape_value_cow_variant_matches_whether_escaping_was_needed() {
        assert!(matches!(escape_value("plain"), std::borrow::Cow::Borrowed(_)));
        assert!(matches!(escape_value("a,b"), std::borrow::Cow::Owned(_)));
        assert!(matches!(escape_value("a\"b"), std::borrow::Cow::Owned(_)));
        // \n/\r alone (no comma or quote) still needs stripping -> Owned, but is
        // NOT wrapped in surrounding quotes (needs_quote stays false).
        let newline_only = escape_value("a\nb");
        assert!(matches!(newline_only, std::borrow::Cow::Owned(_)));
        assert_eq!(newline_only, "ab");
    }
}
