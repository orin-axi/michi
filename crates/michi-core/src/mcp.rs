//! MCP `CallToolResult` mapping — the shape a tool call returns to an MCP client.

use crate::audience::Audience;

/// One text content block. Wire-conformant with MCP's text content shape.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "ContentBlockWire", from = "ContentBlockWire"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct ContentBlock {
    /// The block's text content.
    pub text: String,
    /// Which surface(s) this block is meant for.
    pub audience: Vec<Audience>,
}

impl ContentBlock {
    /// Create a new ContentBlock.
    pub fn new(text: impl Into<String>, audience: Vec<Audience>) -> Self {
        Self { text: text.into(), audience }
    }
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
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(into = "CallToolResultWire", from = "CallToolResultWire"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub struct CallToolResult {
    /// Text content blocks.
    pub content: Vec<ContentBlock>,
    /// Whether this is a tool execution error.
    pub is_error: bool,
    /// The same data as `content[0]`, as a JSON string.
    pub structured_content: String,
}

impl CallToolResult {
    /// Create a new CallToolResult.
    pub fn new(content: Vec<ContentBlock>, is_error: bool, structured_content: impl Into<String>) -> Self {
        Self { content, is_error, structured_content: structured_content.into() }
    }
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
    fn call_tool_result_is_constructible() {
        let r = CallToolResult {
            content: vec![ContentBlock { text: "body".to_string(), audience: vec![Audience::Assistant] }],
            is_error: false,
            structured_content: "{}".to_string(),
        };
        assert_eq!(r.content.len(), 1);
        assert!(!r.is_error);
    }
}
