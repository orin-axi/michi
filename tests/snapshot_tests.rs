use michi::empty::empty_state_with_hints;
use michi::hints::Hint;
use michi::kv::{render_kv, KvItem, KvValue};
use michi::response::{AgentResponse, OutputFormat};
use michi::status::{Health, StatusItem, StatusResponse};
use michi::toon::{render_toon, ToonOptions, Value};

#[test]
fn snapshot_toon_basic_list() {
    let opts = ToonOptions {
        type_name: "issue".into(),
        fields: vec!["number".into(), "title".into(), "state".into()],
        rows: vec![
            vec![Value::Int(42), Value::Str("Fix login redirect".into()), Value::Str("open".into())],
            vec![Value::Int(43), Value::Str("Add dark mode".into()), Value::Str("open".into())],
            vec![Value::Int(44), Value::Str("Update deps, bump major".into()), Value::Str("closed".into())],
        ],
        total_count: Some(47),
        hints: vec![
            Hint::new("Call get_issue with number=<number> for full detail"),
            Hint::new("Call list_issues with state=open to filter"),
        ],
        ..Default::default()
    };
    insta::assert_snapshot!(render_toon(&opts));
}

#[test]
fn snapshot_toon_empty_state() {
    let out = empty_state_with_hints("issue", &[Hint::new("Try list_issues with a broader filter")]);
    insta::assert_snapshot!(out);
}

#[test]
fn snapshot_kv_single_item() {
    let items = vec![
        KvItem { key: "id".into(), value: KvValue::Text("abc-123".into()) },
        KvItem { key: "title".into(), value: KvValue::Text("Fix login".into()) },
        KvItem { key: "state".into(), value: KvValue::Text("open".into()) },
        KvItem { key: "count".into(), value: KvValue::Int(3) },
    ];
    insta::assert_snapshot!(render_kv(&items, None, &[]));
}

#[test]
fn snapshot_kv_column_alignment() {
    let items = vec![
        KvItem { key: "id".into(), value: KvValue::Int(1) },
        KvItem { key: "description".into(), value: KvValue::Text("A longer field value".into()) },
        KvItem { key: "x".into(), value: KvValue::Bool(true) },
    ];
    insta::assert_snapshot!(render_kv(&items, None, &[]));
}

#[test]
fn snapshot_status_mixed_health() {
    let resp = StatusResponse::new(
        "my-tool",
        "does things",
        vec![
            StatusItem { key: "index".into(), value: KvValue::Text("ready".into()), health: Some(Health::Ok) },
            StatusItem {
                key: "cache".into(),
                value: KvValue::Text("warm".into()),
                health: Some(Health::Degraded("near limit".into())),
            },
            StatusItem {
                key: "queue".into(),
                value: KvValue::Text("down".into()),
                health: Some(Health::Error("disconnected".into())),
            },
        ],
    );
    insta::assert_snapshot!(resp.render());
}

#[test]
fn snapshot_agent_response_full() {
    let out = AgentResponse::new("issue")
        .items(vec![], &[])
        .total_count(0)
        .hint("Try list_issues with state=open")
        .hint("Try list_issues with a different label")
        .render(OutputFormat::Text);
    insta::assert_snapshot!(out);
}

#[test]
fn snapshot_call_tool_result_kv() {
    let r = AgentResponse::new("issue")
        .kv_items(vec![
            KvItem { key: "id".into(), value: KvValue::Text("abc-123".into()) },
            KvItem { key: "state".into(), value: KvValue::Text("open".into()) },
        ])
        .human_content("Issue abc-123 is currently open.");
    let result = r.to_call_tool_result();
    insta::assert_debug_snapshot!(result);
}
