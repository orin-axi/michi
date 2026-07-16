//! MCP `CallToolResult` mapping — the shape a tool call actually returns to
//! an MCP client. Always compiled: this is pure struct construction, no new
//! dependencies, so there's no reason to gate it behind a feature flag.
//!
//! michi does not know about the rest of the MCP protocol (no JSON-RPC, no
//! tool registration, no server bootstrapping — see `docs/01-spec.md`'s
//! Non-goals). This module owns exactly one thing: turning an already-built
//! [`crate::response::AgentResponse`] into the `content`/`isError`/
//! `structuredContent` shape MCP's `tools/call` response expects.

/// Which surface a [`ContentBlock`] is meant for. Mirrors MCP's
/// `annotations.audience`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Audience {
    /// The compact, token-efficient surface — what michi renders today.
    Assistant,
    /// A human-readable surface, supplied by the caller. michi does not
    /// generate this text itself (see this crate's Non-goals: no
    /// display-format Markdown) — it only carries it correctly through to
    /// the protocol shape when a caller has one.
    User,
}

/// One text content block, tagged with its intended audience.
#[derive(Debug, Clone, PartialEq)]
pub struct ContentBlock {
    /// The block's text content.
    pub text: String,
    /// Which surface this block is meant for.
    pub audience: Audience,
}

/// The MCP `CallToolResult` shape: what a tool call returns to a client.
/// Built via [`crate::response::AgentResponse::to_call_tool_result`], never
/// hand-constructed by a caller.
#[derive(Debug, Clone, PartialEq)]
pub struct CallToolResult {
    /// Text content blocks — the primary `assistant`-audience block first,
    /// then an optional `user`-audience block if the caller supplied one.
    pub content: Vec<ContentBlock>,
    /// Whether this is a tool execution error, per MCP's error-reporting
    /// model (`isError: true` in the result, not a JSON-RPC protocol error —
    /// see MCP's tools spec, "Tool Execution Errors").
    pub is_error: bool,
    /// The same data as `content[0]`, as a JSON string — MCP's
    /// `structuredContent` companion. Always populated: michi already builds
    /// this JSON for `AgentResponse::render(OutputFormat::Json)`, so
    /// including it costs nothing and gives a JSON-aware client a typed
    /// alternative to parsing the compact text.
    pub structured_content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_block_carries_text_and_audience() {
        let b = ContentBlock { text: "hello".to_string(), audience: Audience::Assistant };
        assert_eq!(b.text, "hello");
        assert_eq!(b.audience, Audience::Assistant);
    }

    #[test]
    fn call_tool_result_is_constructible() {
        let r = CallToolResult {
            content: vec![ContentBlock { text: "body".to_string(), audience: Audience::Assistant }],
            is_error: false,
            structured_content: "{}".to_string(),
        };
        assert_eq!(r.content.len(), 1);
        assert!(!r.is_error);
        assert_eq!(r.structured_content, "{}");
    }
}
