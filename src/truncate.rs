/// Result of a truncation operation.
#[derive(Debug, Clone, PartialEq)]
pub struct Truncated {
    /// The truncated (or original) content string.
    pub content: String,
    /// Original byte length of the input.
    pub original_len: usize,
    /// Whether truncation actually occurred.
    pub was_truncated: bool,
}

/// Truncate `content` to at most `max_chars` Unicode scalar values, appending
/// an agent-readable suffix when truncation occurs.
///
/// The suffix pattern is `" ({n} chars truncated — use {hint})"`. Uses char
/// boundaries — never splits a Unicode scalar.
///
/// Returns the original string unchanged when `content.chars().count() <= max_chars`.
#[must_use]
pub fn truncate(content: &str, max_chars: usize, hint: &str) -> Truncated {
    let char_count = content.chars().count();
    if char_count <= max_chars {
        return Truncated { content: content.to_string(), original_len: content.len(), was_truncated: false };
    }

    let suffix = format!(" ({} chars truncated — use {})", char_count, hint);
    let suffix_chars = suffix.chars().count();
    let keep_chars = max_chars.saturating_sub(suffix_chars);

    let byte_end = content.char_indices().nth(keep_chars).map(|(i, _)| i).unwrap_or(content.len());

    let mut result = String::with_capacity(byte_end + suffix.len());
    result.push_str(&content[..byte_end]);
    result.push_str(&suffix);

    // Hard-cap to max_chars via char boundaries in case the suffix alone
    // exceeds max_chars (e.g. very small max_chars or long hint string).
    if result.chars().count() > max_chars {
        let cap_byte = result.char_indices().nth(max_chars).map(|(i, _)| i).unwrap_or(result.len());
        result.truncate(cap_byte);
    }

    Truncated { content: result, original_len: content.len(), was_truncated: true }
}

/// Truncate content for inline use (e.g. inside a TOON field).
///
/// Returns the final string directly.
#[must_use]
pub fn truncate_inline(content: &str, max_chars: usize, hint: &str) -> String {
    truncate(content, max_chars, hint).content
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_content_not_truncated() {
        let t = truncate("hello", 100, "full=true");
        assert!(!t.was_truncated);
        assert_eq!(t.content, "hello");
        assert_eq!(t.original_len, 5);
    }

    #[test]
    fn long_content_is_truncated() {
        let content = "a".repeat(200);
        let t = truncate(&content, 50, "full=true");
        assert!(t.was_truncated);
        assert!(t.content.chars().count() <= 50);
        assert!(t.content.contains("chars truncated"));
        assert!(t.content.contains("full=true"));
    }

    #[test]
    fn truncation_respects_char_boundaries() {
        let content = "こんにちは世界！これはテストです。";
        let t = truncate(content, 10, "full=true");
        assert!(std::str::from_utf8(t.content.as_bytes()).is_ok());
        if t.was_truncated {
            assert!(t.content.chars().count() <= 10);
        }
    }

    #[test]
    fn truncate_inline_returns_string() {
        let content = "x".repeat(200);
        let result = truncate_inline(&content, 30, "full=true");
        assert!(result.chars().count() <= 30);
    }

    #[test]
    fn exact_length_not_truncated() {
        let t = truncate("hello", 5, "full=true");
        assert!(!t.was_truncated);
        assert_eq!(t.content, "hello");
    }

    #[test]
    fn suffix_contains_original_char_count() {
        let content = "a".repeat(100);
        let t = truncate(&content, 40, "full=true");
        assert!(t.was_truncated);
        assert!(t.content.contains("100 chars truncated"));
    }
}
