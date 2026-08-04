//! AgentResponse builder and MCP CallToolResult assembly example.
//! Run with: `cargo run --example agent_response`

use michi::kv::{KvItem, KvValue};
use michi::response::{AgentResponse, OutputFormat};

fn main() {
    let response = AgentResponse::new("issue")
        .kv_items(vec![
            KvItem { key: "id".into(), value: KvValue::Text("issue-51815".into()) },
            KvItem { key: "title".into(), value: KvValue::Text("[Bug]: Telegram plugin".into()) },
            KvItem { key: "state".into(), value: KvValue::Text("open".into()) },
        ])
        .hint("Run `gh-axi issue view <number>` to view an issue")
        .human_content("Issue #51815 is currently open in the Telegram plugin module.");

    println!("=== Plain Text Output ===");
    println!("{}", response.render(OutputFormat::Text));

    println!("=== MCP CallToolResult ===");
    let mcp_result = response.to_call_tool_result();
    println!("Is error: {}", mcp_result.is_error);
    println!("Content blocks count: {}", mcp_result.content.len());
    for block in &mcp_result.content {
        println!("  Block (audience: {:?}): {}", block.audience, block.text);
    }
}
