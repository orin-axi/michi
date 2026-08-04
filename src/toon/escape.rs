/// Escape a scalar value for TOON row output.
///
/// Values containing commas or double-quotes are wrapped in double-quotes
/// with internal double-quotes escaped as `\"`. Embedded newlines and
/// carriage returns are stripped, since TOON v0.1 does not support multi-line
/// cell values. Empty values are returned as-is (the comma delimiter is
/// still emitted by the caller).
pub(crate) fn escape_value(v: &str) -> std::borrow::Cow<'_, str> {
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
///
/// Unlike [`escape_value`], which only quotes when the value contains a
/// delimiter, this always quotes — for free-text fields (e.g. a
/// human-readable failure reason) where the row shape should stay
/// predictable regardless of incidental comma/quote content.
pub(crate) fn escape_value_quoted(v: &str) -> String {
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
    fn value_with_newline_is_stripped() {
        assert_eq!(escape_value("line\nbreak"), "linebreak");
    }

    #[test]
    fn value_with_cr_is_stripped() {
        assert_eq!(escape_value("line\rend"), "lineend");
    }

    #[test]
    fn value_with_newline_and_comma_is_stripped_and_quoted() {
        assert_eq!(escape_value("a,b\nc"), r#""a,bc""#);
    }

    #[test]
    fn quoted_wraps_plain_value_that_needs_no_escaping() {
        assert_eq!(escape_value_quoted("User 'ghost' not found"), r#""User 'ghost' not found""#);
    }

    #[test]
    fn quoted_escapes_internal_double_quotes() {
        assert_eq!(escape_value_quoted(r#"say "hi""#), r#""say \"hi\"""#);
    }

    #[test]
    fn quoted_strips_embedded_newlines() {
        assert_eq!(escape_value_quoted("line\nbreak"), r#""linebreak""#);
    }
}


