#![deny(unsafe_code)]
#![warn(missing_docs)]

//! # michi-truncate
//!
//! UTF-8 char-boundary safe content truncation primitives for michi and AXI.

/// Result of a truncation operation.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub struct Truncated {
    /// The truncated (or original) content string.
    pub content: String,
    /// Original byte length of the input.
    pub original_len: usize,
    /// Whether truncation actually occurred.
    pub was_truncated: bool,
    /// The truncation signal text alone (e.g. `"(N chars truncated — use
    /// full=true)"`), separate from `content`. `None` when not truncated.
    pub signal: Option<String>,
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
    // Byte length is always >= char count, so if it already fits within
    // max_chars the char count does too — skip the full UTF-8 scan.
    if content.len() <= max_chars {
        return Truncated {
            content: content.to_string(),
            original_len: content.len(),
            was_truncated: false,
            signal: None,
        };
    }

    let char_count = content.chars().count();
    if char_count <= max_chars {
        return Truncated {
            content: content.to_string(),
            original_len: content.len(),
            was_truncated: false,
            signal: None,
        };
    }

    let suffix = format!(" ({char_count} chars truncated — use {hint})");
    let suffix_chars = suffix.chars().count();
    let keep_chars = max_chars.saturating_sub(suffix_chars);

    let byte_end = content.char_indices().nth(keep_chars).map_or(content.len(), |(i, _)| i);

    let mut result = String::with_capacity(byte_end + suffix.len());
    result.push_str(&content[..byte_end]);
    result.push_str(&suffix);

    // Hard-cap to max_chars via char boundaries in case the suffix alone
    // exceeds max_chars (e.g. very small max_chars or long hint string).
    if result.chars().count() > max_chars {
        let cap_byte = result.char_indices().nth(max_chars).map_or(result.len(), |(i, _)| i);
        result.truncate(cap_byte);
    }

    Truncated {
        content: result,
        original_len: content.len(),
        was_truncated: true,
        signal: Some(suffix.trim_start().to_string()),
    }
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

    #[test]
    fn large_short_content_not_truncated() {
        let content = "a".repeat(5000);
        let t = truncate(&content, 10_000, "full=true");
        assert!(!t.was_truncated);
        assert_eq!(t.content, content);
        assert_eq!(t.original_len, 5000);
    }

    #[test]
    fn signal_is_populated_when_truncated() {
        let content = "a".repeat(200);
        let t = truncate(&content, 50, "full=true");
        assert!(t.was_truncated);
        let signal = t.signal.as_deref().expect("signal present when truncated");
        assert!(signal.contains("200 chars truncated"));
        assert!(signal.contains("full=true"));
        assert_eq!(signal, "(200 chars truncated — use full=true)");
    }

    #[test]
    fn signal_is_none_when_not_truncated() {
        let t = truncate("hello", 100, "full=true");
        assert!(!t.was_truncated);
        assert_eq!(t.signal, None);
    }

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn test_auto_traits() {
        assert_send_sync_static::<Truncated>();
    }
}
