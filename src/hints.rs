/// A contextual usage hint for an agent.
///
/// Hints are surfaced in `help[N]:` blocks at the end of TOON or kv responses.
/// They teach the agent what to call next.
#[derive(Debug, Clone, PartialEq)]
pub struct Hint(pub String);

impl Hint {
    /// Create a hint from any string-like value.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The raw hint string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for Hint {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

impl From<String> for Hint {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Render a `help[N]:` block from a slice of hints.
///
/// Returns an empty string when `hints` is empty.
#[must_use]
pub fn render_hints(hints: &[Hint]) -> String {
    if hints.is_empty() {
        return String::new();
    }
    let capacity = 12 + hints.len() * 50;
    let mut out = String::with_capacity(capacity);
    out.push_str("help[");
    out.push_str(&hints.len().to_string());
    out.push_str("]:\n");
    for hint in hints {
        out.push_str("  ");
        out.push_str(hint.as_str());
        out.push('\n');
    }
    out
}

/// Append a `help[N]:` block to an existing string in-place.
///
/// No-op when `hints` is empty.
pub fn append_hints(out: &mut String, hints: &[Hint]) {
    if hints.is_empty() {
        return;
    }
    out.push_str(&render_hints(hints));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_hint_renders_correctly() {
        let hints = [Hint::new("Call get_item with id=<id>")];
        assert_eq!(render_hints(&hints), "help[1]:\n  Call get_item with id=<id>\n");
    }

    #[test]
    fn multiple_hints() {
        let hints = [Hint::new("hint one"), Hint::new("hint two")];
        assert_eq!(render_hints(&hints), "help[2]:\n  hint one\n  hint two\n");
    }

    #[test]
    fn empty_hints_returns_empty() {
        assert_eq!(render_hints(&[]), "");
    }

    #[test]
    fn append_hints_modifies_string() {
        let mut s = "issue[0]{}:\n".to_string();
        append_hints(&mut s, &[Hint::new("try again")]);
        assert!(s.ends_with("help[1]:\n  try again\n"));
    }

    #[test]
    fn append_hints_noop_when_empty() {
        let mut s = "base".to_string();
        append_hints(&mut s, &[]);
        assert_eq!(s, "base");
    }

    #[test]
    fn hint_from_str_ref() {
        let h: Hint = "test".into();
        assert_eq!(h.as_str(), "test");
    }

    #[test]
    fn hint_from_string() {
        let h: Hint = String::from("test").into();
        assert_eq!(h.as_str(), "test");
    }
}
