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
    assert_eq!(out, "issue[0]{}:\ntotalCount: 0\nhelp[1]:\n  Try list_issues with a broader filter\n");
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

#[test]
fn mixed_numeric_row_uses_comma_separators() {
    let opts = ToonOptions {
        type_name: "point".into(),
        fields: vec!["x".into(), "y".into(), "z".into()],
        rows: vec![vec![Value::Int(-7), Value::Float(2.5), Value::Int(0)]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    assert!(out.contains("  -7,2.5,0\n"), "expected comma-separated numeric row, got: {out}");
}

#[test]
fn float_value_renders() {
    let opts = ToonOptions {
        type_name: "measurement".into(),
        fields: vec!["value".into()],
        rows: vec![vec![Value::Float(3.14)]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    assert!(out.contains("3.14"), "expected 3.14 in output, got: {out}");
}

#[test]
fn quote_in_value_is_escaped() {
    let opts = ToonOptions {
        type_name: "item".into(),
        fields: vec!["description".into()],
        rows: vec![vec![Value::Str(r#"say "hello""#.into())]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    assert!(out.contains(r#""say \"hello\"""#), "expected escaped quotes in output, got: {out}");
}

#[test]
fn newline_in_value_is_stripped() {
    let opts = ToonOptions {
        type_name: "item".into(),
        fields: vec!["body".into()],
        rows: vec![vec![Value::Str("line1\nline2".into())]],
        total_count: None,
        hints: vec![],
    };
    let out = render_toon(&opts);
    // Embedded newlines must be stripped so each row stays a single physical line
    assert!(out.contains("line1line2"), "expected stripped newline value, got: {out}");
    assert!(!out.contains('"'), "value should not need quoting once newline is stripped, got: {out}");
}
