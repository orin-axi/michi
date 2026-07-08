use michi::empty::empty_state_with_hints;
use michi::hints::Hint;
use michi::kv::{render_kv, KvItem, KvValue};
use michi::response::{AgentResponse, OutputFormat};
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
            "Call get_issue with number=<number> for full detail".into(),
            "Call list_issues with state=open to filter".into(),
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
        KvItem { key: "id".into(), value: KvValue::Str("abc-123".into()) },
        KvItem { key: "title".into(), value: KvValue::Str("Fix login".into()) },
        KvItem { key: "state".into(), value: KvValue::Str("open".into()) },
        KvItem { key: "count".into(), value: KvValue::Int(3) },
    ];
    insta::assert_snapshot!(render_kv(&items));
}

#[test]
fn snapshot_agent_response_full() {
    let out = AgentResponse::new("issue[0]{}:\ntotalCount: 0\n")
        .with_hint(Hint::new("Try list_issues with state=open"))
        .with_hint(Hint::new("Try list_issues with a different label"))
        .render(OutputFormat::Text);
    insta::assert_snapshot!(out);
}
