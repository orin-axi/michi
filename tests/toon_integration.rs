use michi::toon::{render_toon, ToonOptions, Value};

#[test]
fn renders_basic_list() {
    let opts = ToonOptions {
        type_name: "issue".into(),
        fields: vec!["number".into(), "title".into(), "state".into()],
        rows: vec![
            vec![Value::Int(42), Value::Str("Fix login".into()), Value::Str("open".into())],
            vec![Value::Int(43), Value::Str("Add dark mode".into()), Value::Str("open".into())],
        ],
        total_count: Some(47),
        hints: vec!["Call get_issue with number=<number> for full detail".into()],
    };
    let out = render_toon(&opts);
    assert_eq!(
        out,
        "issue[2]{number,title,state}:\n  42,Fix login,open\n  43,Add dark mode,open\ntotalCount: 47\nhelp[1]:\n  Call get_issue with number=<number> for full detail\n"
    );
}

#[test]
fn renders_empty_state() {
    let opts = ToonOptions {
        type_name: "issue".into(),
        fields: vec![],
        rows: vec![],
        total_count: Some(0),
        hints: vec!["Try list_issues with a broader filter".into()],
    };
    let out = render_toon(&opts);
    assert_eq!(
        out,
        "issue[0]{}:\ntotalCount: 0\nhelp[1]:\n  Try list_issues with a broader filter\n"
    );
}

#[test]
fn escapes_comma_in_value() {
    let opts = ToonOptions {
        type_name: "item".into(),
        fields: vec!["name".into()],
        rows: vec![vec![Value::Str("Update deps, bump major".into())]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    assert!(out.contains(r#""Update deps, bump major""#));
}

#[test]
fn null_value_renders_as_empty_field() {
    let opts = ToonOptions {
        type_name: "item".into(),
        fields: vec!["a".into(), "b".into()],
        rows: vec![vec![Value::Str("x".into()), Value::Null]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    assert!(out.contains("  x,\n"));
}

#[test]
fn bool_values_render_as_true_false() {
    let opts = ToonOptions {
        type_name: "flag".into(),
        fields: vec!["enabled".into(), "visible".into()],
        rows: vec![vec![Value::Bool(true), Value::Bool(false)]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    assert!(out.contains("  true,false\n"));
}

#[test]
fn no_total_count_when_none() {
    let opts = ToonOptions {
        type_name: "item".into(),
        fields: vec!["id".into()],
        rows: vec![vec![Value::Int(1)]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    assert!(!out.contains("totalCount"));
}

#[test]
fn multiple_hints_render_correctly() {
    let opts = ToonOptions {
        type_name: "item".into(),
        fields: vec!["id".into()],
        rows: vec![],
        total_count: Some(0),
        hints: vec!["hint one".into(), "hint two".into()],
    };
    let out = render_toon(&opts);
    assert!(out.contains("help[2]:\n  hint one\n  hint two\n"));
}
