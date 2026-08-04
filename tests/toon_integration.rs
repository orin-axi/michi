use michi::toon::{render_toon, ToonOptions, Value};

#[test]
fn renders_basic_list() {
    let opts = ToonOptions::new(
        "issue",
        vec!["number".into(), "title".into(), "state".into()],
        vec![
            vec![Value::Int(42), Value::Str("Fix login".into()), Value::Str("open".into())],
            vec![Value::Int(43), Value::Str("Add dark mode".into()), Value::Str("open".into())],
        ],
    )
    .total_count(Some(47))
    .hints(vec!["Call get_issue with number=<number> for full detail".to_string()]);

    let out = render_toon(&opts);
    assert_eq!(
        out,
        "issue[2]{number,title,state}:\n  42,Fix login,open\n  43,Add dark mode,open\ntotalCount: 47\nhelp[1]:\n  Call get_issue with number=<number> for full detail\n"
    );
}

#[test]
fn renders_empty_state() {
    let opts = ToonOptions::new("issue", vec![], vec![])
        .total_count(Some(0))
        .hints(vec!["Try list_issues with a broader filter".to_string()]);
    let out = render_toon(&opts);
    assert_eq!(out, "issue[0]{}:\ntotalCount: 0\nhelp[1]:\n  Try list_issues with a broader filter\n");
}

#[test]
fn escapes_comma_in_value() {
    let opts = ToonOptions::new("item", vec!["name".into()], vec![vec![Value::Str("Update deps, bump major".into())]]);
    let out = render_toon(&opts);
    assert!(out.contains(r#""Update deps, bump major""#));
}

#[test]
fn null_value_renders_as_empty_field() {
    let opts = ToonOptions::new("item", vec!["a".into(), "b".into()], vec![vec![Value::Str("x".into()), Value::Null]]);
    let out = render_toon(&opts);
    assert!(out.contains("  x,\n"));
}

#[test]
fn bool_values_render_as_true_false() {
    let opts = ToonOptions::new(
        "flag",
        vec!["enabled".into(), "visible".into()],
        vec![vec![Value::Bool(true), Value::Bool(false)]],
    );
    let out = render_toon(&opts);
    assert!(out.contains("  true,false\n"));
}

#[test]
fn no_total_count_when_none() {
    let opts = ToonOptions::new("item", vec!["id".into()], vec![vec![Value::Int(1)]]);
    let out = render_toon(&opts);
    assert!(!out.contains("totalCount"));
}

#[test]
fn multiple_hints_render_correctly() {
    let opts = ToonOptions::new("item", vec!["id".into()], vec![])
        .total_count(Some(0))
        .hints(vec!["hint one".to_string(), "hint two".to_string()]);
    let out = render_toon(&opts);
    assert!(out.contains("help[2]:\n  hint one\n  hint two\n"));
}

#[test]
fn mixed_numeric_row_uses_comma_separators() {
    let opts = ToonOptions::new(
        "point",
        vec!["x".into(), "y".into(), "z".into()],
        vec![vec![Value::Int(-7), Value::Float(2.5), Value::Int(0)]],
    );
    let out = render_toon(&opts);
    assert!(out.contains("  -7,2.5,0\n"), "expected comma-separated numeric row, got: {out}");
}

#[test]
fn float_value_renders() {
    let opts = ToonOptions::new("measurement", vec!["value".into()], vec![vec![Value::Float(3.14)]]);
    let out = render_toon(&opts);
    assert!(out.contains("3.14"), "expected 3.14 in output, got: {out}");
}

#[test]
fn quote_in_value_is_escaped() {
    let opts = ToonOptions::new("item", vec!["description".into()], vec![vec![Value::Str(r#"say "hello""#.into())]]);
    let out = render_toon(&opts);
    assert!(out.contains(r#""say \"hello\"""#), "expected escaped quotes in output, got: {out}");
}

#[test]
fn newline_in_value_is_stripped() {
    let opts = ToonOptions::new("item", vec!["body".into()], vec![vec![Value::Str("line1\nline2".into())]]);
    let out = render_toon(&opts);
    assert!(out.contains("line1line2"), "expected stripped newline value, got: {out}");
    assert!(!out.contains('"'), "value should not need quoting once newline is stripped, got: {out}");
}

#[test]
fn long_cell_value_is_truncated_per_max_cell_len() {
    let long_title = "x".repeat(300);
    let opts = ToonOptions::new("issue", vec!["title".to_string()], vec![vec![Value::Str(long_title.into())]])
        .max_cell_len(50);
    let out = render_toon(&opts);
    assert!(out.contains("chars truncated"), "expected truncation signal, got: {out}");
    let row_line = out.lines().nth(1).expect("row line exists");
    assert!(row_line.chars().count() <= 50 + 40, "row line too long: {row_line}");
}

#[test]
fn short_cell_value_is_not_truncated() {
    let opts = ToonOptions::new("issue", vec!["title".to_string()], vec![vec![Value::Str("short".into())]]);
    let out = render_toon(&opts);
    assert!(out.contains("short"));
    assert!(!out.contains("chars truncated"));
}

#[test]
fn hints_field_accepts_hint_type() {
    let opts = ToonOptions::new("issue", vec![], vec![]).hints(vec!["do this".to_string()]);
    let out = render_toon(&opts);
    assert!(out.contains("help[1]:\n  do this\n"));
}

#[test]
fn crate_root_reexports_are_reachable() {
    let _: fn(&[michi::kv::KvItem], Option<usize>, &[michi::Hint]) -> String = michi::render_kv;
    let _ = michi::empty_state("t");
    let _: fn(Option<String>) -> michi::AlreadyDone = michi::already_done;
    let _: fn(&str, &str, &[String]) -> String = michi::render_already_done;
    let _ = michi::RetryConfig::default();
    let _: fn(&str) -> Option<std::time::Duration> = michi::parse_retry_after;
    let _: fn(&michi::resilience::RetryConfig, u32, f64, Option<std::time::Duration>) -> Option<std::time::Duration> =
        michi::next_retry_delay;
    let _: fn(&str, usize, &str) -> michi::Truncated = michi::truncate;
    let _: fn(&str, usize, &str) -> String = michi::truncate_inline;
    let _ = michi::RecoveryHint::new("t");
    let _ = michi::StatusResponse::new("t", "d", vec![]);
}

#[test]
#[cfg(feature = "serde")]
fn list_builds_toon_options_from_serializable_struct_slice() {
    #[derive(serde::Serialize)]
    struct Issue {
        number: u64,
        title: String,
        state: String,
    }

    let issues = vec![Issue { number: 51815, title: "[Bug]: Telegram plugin".to_string(), state: "open".to_string() }];
    let out = michi::toon::list("issues", &issues).expect("list failed");
    assert!(out.starts_with("issues[1]{number,title,state}:\n"), "got: {out}");
    assert!(out.contains("51815,[Bug]: Telegram plugin,open"), "got: {out}");
}

#[test]
#[cfg(feature = "serde")]
fn list_handles_empty_slice() {
    #[derive(serde::Serialize)]
    struct Empty {
        x: i32,
    }
    let items: Vec<Empty> = vec![];
    let out = michi::toon::list("nothing", &items).expect("list failed");
    // Empty slice: header with 0 rows, no data lines
    assert!(out.starts_with("nothing[0]"), "got: {out}");
    assert!(!out.contains('\n') || out.lines().count() <= 1, "got: {out}");
}

#[test]
#[cfg(feature = "serde")]
fn list_stringifies_nested_values_losslessly() {
    #[derive(serde::Serialize)]
    struct WithNested {
        id: u64,
        tags: Vec<String>,
    }
    let items = vec![WithNested { id: 1, tags: vec!["a".to_string(), "b".to_string()] }];
    let out = michi::toon::list("t", &items).expect("list failed");
    // Nested array is stringified then quoted (commas inside require quoting).
    // Expected cell: "[\"a\",\"b\"]"
    assert!(out.contains(r#""[\"a\",\"b\"]""#), "nested value must be stringified and quoted, got: {out}");
}
