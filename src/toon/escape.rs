/// Escape a scalar value for TOON row output.
///
/// Values containing commas, double-quotes, or newlines are wrapped in
/// double-quotes with internal double-quotes escaped as `\"`. Empty values
/// are returned as-is (the comma delimiter is still emitted by the caller).
pub(crate) fn escape_value(v: &str) -> std::borrow::Cow<'_, str> {
    if v.is_empty() {
        return std::borrow::Cow::Borrowed(v);
    }
    if v.contains(',') || v.contains('"') || v.contains('\n') || v.contains('\r') {
        let mut out = String::with_capacity(v.len() * 2 + 2);
        out.push('"');
        for ch in v.chars() {
            if ch == '"' {
                out.push('\\');
            }
            out.push(ch);
        }
        out.push('"');
        std::borrow::Cow::Owned(out)
    } else {
        std::borrow::Cow::Borrowed(v)
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
    fn value_with_newline_is_quoted() {
        assert_eq!(escape_value("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn value_with_cr_is_quoted() {
        assert_eq!(escape_value("line\rend"), "\"line\rend\"");
    }
}
