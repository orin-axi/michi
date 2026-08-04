/// A contextual usage hint for an agent.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
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

impl std::fmt::Display for Hint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Render a `help[N]:` block from a slice of hints.
#[must_use]
pub fn render_hints(hints: &[Hint]) -> String {
    if hints.is_empty() {
        return String::new();
    }
    let mut out = String::with_capacity(12 + hints.len() * 50);
    append_hints(&mut out, hints);
    out
}

/// Append a `help[N]:` block to an existing string in-place.
pub fn append_hints(out: &mut String, hints: &[Hint]) {
    if hints.is_empty() {
        return;
    }
    out.push_str("help[");
    out.push_str(&hints.len().to_string());
    out.push_str("]:\n");
    for hint in hints {
        out.push_str("  ");
        out.push_str(hint.as_str());
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_hint_renders_correctly() {
        let hints = [Hint::new("Call get_item with id=<id>")];
        assert_eq!(render_hints(&hints), "help[1]:\n  Call get_item with id=<id>\n");
    }
}
