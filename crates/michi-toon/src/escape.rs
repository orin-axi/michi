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
}
