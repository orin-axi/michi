//! MCP `CallToolResult` mapping — the shape a tool call actually returns to
//! an MCP client. Always compiled: this is pure struct construction, no new
//! dependencies, so there's no reason to gate it behind a feature flag.
//!
//! michi does not know about the rest of the MCP protocol (no JSON-RPC, no
//! tool registration, no server bootstrapping, no `outputSchema` validation —
//! see `docs/spec/01-overview-and-setup.md`'s Non-goals). This module owns exactly one thing:
//! turning an already-built [`crate::response::AgentResponse`] into the
//! `content`/`isError`/`structuredContent` shape MCP's `tools/call` response
//! expects — including the real wire-format details (the `"type": "text"`
//! discriminator, `annotations.audience` nesting, camelCase field names)
//! under the `serde` feature and the NAPI boundary, not just an internal
//! Rust-shaped approximation of them.

/// Which surface a [`ContentBlock`] is meant for. Mirrors MCP's
/// `annotations.audience` — an array in the real protocol because one block
/// can target more than one audience; michi always populates exactly one
/// element per block today (see [`ContentBlock::audience`]), but the field
/// is a `Vec` so no translation is needed at the serialization boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Audience {
    /// The compact, token-efficient surface — what michi renders today.
    Assistant,
    /// A human-readable surface, supplied by the caller. michi does not
    /// generate this text itself (see this crate's Non-goals: no
    /// display-format Markdown) — it only carries it correctly through to
    /// the protocol shape when a caller has one.
    User,
}

/// One text content block. Wire-conformant with MCP's text content shape —
/// `{"type": "text", "text": "...", "annotations": {"audience": [...]}}` —
/// under the `serde` feature; NAPI's `JsContentBlock` (`src/napi.rs`)
/// produces the identical shape independently for JS callers.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "ContentBlockWire", from = "ContentBlockWire"))]
pub struct ContentBlock {
    /// The block's text content.
    pub text: String,
    /// Which surface(s) this block is meant for. Always one element today
    /// (assistant XOR user) — a `Vec` because MCP's `annotations.audience`
    /// is an array; see this type's own doc comment.
    pub audience: Vec<Audience>,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AnnotationsWire {
    audience: Vec<Audience>,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
struct ContentBlockWire {
    #[serde(rename = "type")]
    kind: String,
    text: String,
    annotations: AnnotationsWire,
}

#[cfg(feature = "serde")]
impl From<ContentBlock> for ContentBlockWire {
    fn from(b: ContentBlock) -> Self {
        Self { kind: "text".to_string(), text: b.text, annotations: AnnotationsWire { audience: b.audience } }
    }
}

#[cfg(feature = "serde")]
impl From<ContentBlockWire> for ContentBlock {
    fn from(w: ContentBlockWire) -> Self {
        Self { text: w.text, audience: w.annotations.audience }
    }
}

/// The MCP `CallToolResult` shape: what a tool call returns to a client.
/// Built via [`crate::response::AgentResponse::to_call_tool_result`], never
/// hand-constructed by a caller.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "CallToolResultWire", from = "CallToolResultWire"))]
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
    /// this JSON for `AgentResponse::render(OutputFormat::Json)`. Only
    /// `hints`/`recovery`/`isError` are genuinely structured within it —
    /// `body` (the rendered TOON/KV text, including `totalCount`) stays a
    /// single embedded string, so a JSON-aware client gains structure over
    /// `content[0].text` for the former but still has to parse the latter.
    /// Populated unconditionally, even for a tool with a declared
    /// `outputSchema` this generic shape won't conform to — michi has no
    /// visibility into any `outputSchema` (confirmed non-goal), so a caller
    /// with one should substitute their own conforming payload instead of
    /// using this field as-is. Kept as a plain `String` (not
    /// `serde_json::Value`) so this always-compiled module doesn't need
    /// `serde_json` as a mandatory dependency; the `serde` feature and NAPI
    /// boundary each independently convert it to a real embedded JSON value
    /// on the wire (see [`CallToolResultWire`], and `src/napi.rs`).
    pub structured_content: String,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CallToolResultWire {
    content: Vec<ContentBlock>,
    is_error: bool,
    structured_content: serde_json::Value,
}

#[cfg(feature = "serde")]
impl From<CallToolResult> for CallToolResultWire {
    fn from(r: CallToolResult) -> Self {
        let structured_content = serde_json::from_str(&r.structured_content).unwrap_or(serde_json::Value::Null);
        Self { content: r.content, is_error: r.is_error, structured_content }
    }
}

#[cfg(feature = "serde")]
impl From<CallToolResultWire> for CallToolResult {
    fn from(w: CallToolResultWire) -> Self {
        let structured_content = serde_json::to_string(&w.structured_content).unwrap_or_default();
        Self { content: w.content, is_error: w.is_error, structured_content }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_block_carries_text_and_audience() {
        let b = ContentBlock { text: "hello".to_string(), audience: vec![Audience::Assistant] };
        assert_eq!(b.text, "hello");
        assert_eq!(b.audience, vec![Audience::Assistant]);
    }

    #[test]
    fn call_tool_result_is_constructible() {
        let r = CallToolResult {
            content: vec![ContentBlock { text: "body".to_string(), audience: vec![Audience::Assistant] }],
            is_error: false,
            structured_content: "{}".to_string(),
        };
        assert_eq!(r.content.len(), 1);
        assert!(!r.is_error);
        assert_eq!(r.structured_content, "{}");
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

    #[test]
    #[cfg(feature = "serde")]
    fn call_tool_result_serializes_and_deserializes() {
        let r = CallToolResult {
            content: vec![ContentBlock { text: "body".to_string(), audience: vec![Audience::Assistant] }],
            is_error: false,
            structured_content: "{}".to_string(),
        };
        let json = serde_json::to_string(&r).expect("serializes");
        let back: CallToolResult = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(r, back);
    }

    #[test]
    #[cfg(feature = "serde")]
    fn call_tool_result_wire_json_matches_mcp_shape() {
        let r = CallToolResult {
            content: vec![ContentBlock { text: "hi".to_string(), audience: vec![Audience::Assistant] }],
            is_error: false,
            structured_content: "{}".to_string(),
        };
        let json = serde_json::to_string(&r).expect("serializes");
        assert_eq!(
            json,
            r#"{"content":[{"type":"text","text":"hi","annotations":{"audience":["assistant"]}}],"isError":false,"structuredContent":{}}"#
        );
    }

    #[test]
    #[cfg(feature = "serde")]
    fn call_tool_result_structured_content_round_trips_as_object_not_double_encoded_string() {
        let r =
            CallToolResult { content: vec![], is_error: false, structured_content: r#"{"totalCount":3}"#.to_string() };
        let json = serde_json::to_string(&r).expect("serializes");
        // structuredContent must be an embedded object — assert the raw JSON has an
        // unescaped, nested object, not a JSON string containing escaped quotes.
        assert!(json.contains(r#""structuredContent":{"totalCount":3}"#), "got: {json}");
        let back: CallToolResult = serde_json::from_str(&json).expect("deserializes");
        assert_eq!(back.structured_content, r#"{"totalCount":3}"#);
    }
}
