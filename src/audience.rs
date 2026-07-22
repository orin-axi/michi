//! Which surface a piece of content is meant for — the model reading it, or
//! a human. Not MCP-specific despite MCP being its first consumer: any
//! response-rendering context that distinguishes agent-facing from
//! human-facing output uses this same type. See
//! [`crate::response::AgentResponse::render_for`] and
//! [`crate::response::AgentResponse::to_call_tool_result`].

/// Which surface a piece of content is meant for. Mirrors MCP's
/// `annotations.audience` — an array in the real protocol because one block
/// can target more than one audience; michi always populates exactly one
/// element per block today (see [`crate::mcp::ContentBlock::audience`]), but
/// the field is a `Vec` so no translation is needed at the serialization
/// boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Audience {
    /// The compact, token-efficient surface — what michi renders today.
    Assistant,
    /// A human-readable surface, supplied by the caller. michi does not
    /// generate this text itself (see this crate's Non-goals: no
    /// display-format Markdown) — it only carries it correctly through to
    /// whichever output the caller asked for.
    User,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audience_is_copy_and_comparable() {
        let a = Audience::Assistant;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(Audience::Assistant, Audience::User);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn audience_serializes_and_deserializes() {
        let a = Audience::Assistant;
        let json = serde_json::to_string(&a).expect("serializes");
        let back: Audience = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(a, back);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn audience_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Audience::Assistant).expect("serializes"), "\"assistant\"");
        assert_eq!(serde_json::to_string(&Audience::User).expect("serializes"), "\"user\"");
    }
}
