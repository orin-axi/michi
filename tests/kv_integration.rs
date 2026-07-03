use michi::kv::{render_kv, KvItem, KvValue};

#[test]
fn single_item_renders() {
    let items = vec![KvItem { key: "id".into(), value: KvValue::Str("abc-123".into()) }];
    assert_eq!(render_kv(&items), "id: abc-123\n");
}

#[test]
fn multiple_items_render_in_order() {
    let items = vec![
        KvItem { key: "id".into(), value: KvValue::Str("abc-123".into()) },
        KvItem { key: "title".into(), value: KvValue::Str("Fix login".into()) },
        KvItem { key: "state".into(), value: KvValue::Str("open".into()) },
        KvItem { key: "count".into(), value: KvValue::Int(3) },
    ];
    let out = render_kv(&items);
    assert_eq!(out, "id: abc-123\ntitle: Fix login\nstate: open\ncount: 3\n");
}

#[test]
fn null_value_leaves_empty_after_colon() {
    let items = vec![KvItem { key: "assignee".into(), value: KvValue::Null }];
    assert_eq!(render_kv(&items), "assignee: \n");
}

#[test]
fn bool_values_render_as_words() {
    let items = vec![
        KvItem { key: "active".into(), value: KvValue::Bool(true) },
        KvItem { key: "archived".into(), value: KvValue::Bool(false) },
    ];
    assert_eq!(render_kv(&items), "active: true\narchived: false\n");
}

#[test]
fn empty_slice_returns_empty_string() {
    assert_eq!(render_kv(&[]), "");
}
