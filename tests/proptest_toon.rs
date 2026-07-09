use michi::toon::{render_toon, ToonOptions, Value};
use proptest::prelude::*;

mod support;
use support::toon_parser::{parse, value_to_string};

/// Cell strategy mixing plain alphanumeric text with strings drawn from a
/// charset that includes comma, double-quote, and newline/CR — the exact
/// characters `escape_value` treats specially — so the round-trip test
/// regularly exercises quoting/escaping, not just the happy path.
fn cell_strategy() -> impl Strategy<Value = String> {
    let escaping_char = prop_oneof![
        Just(','),
        Just('"'),
        Just('\n'),
        Just('\r'),
        proptest::char::range('a', 'z'),
        proptest::char::range('0', '9'),
    ];
    prop_oneof![
        "[a-zA-Z0-9 ]{0,20}",
        proptest::collection::vec(escaping_char, 0..20).prop_map(|chars| chars.into_iter().collect()),
    ]
}

/// Row/field counts and the `total_count` option are generated first, then
/// the cell grid is generated depending on them (`field_count` columns ×
/// `row_count` rows) via `prop_flat_map` — the `proptest!` macro's `x in
/// strategy` bindings can't reference each other directly, so the dependent
/// composition has to happen outside it.
fn doc_strategy() -> impl Strategy<Value = (String, usize, usize, Option<usize>, Vec<Vec<String>>)> {
    ("[a-z][a-z0-9_]{0,10}", 1usize..4, 0usize..5, proptest::option::of(0usize..10_000)).prop_flat_map(
        |(type_name, field_count, row_count, total_count)| {
            proptest::collection::vec(proptest::collection::vec(cell_strategy(), field_count), row_count)
                .prop_map(move |cells| (type_name.clone(), field_count, row_count, total_count, cells))
        },
    )
}

proptest! {
    #[test]
    fn render_toon_output_is_grammar_valid(
        (type_name, field_count, row_count, total_count, cells) in doc_strategy(),
    ) {
        let fields: Vec<String> = (0..field_count).map(|i| format!("f{i}")).collect();
        // Each cell is generated independently per row×field position (rather
        // than reusing one shared value everywhere), so a bug that
        // transposed rows or misaligned columns would actually be caught by
        // the per-position comparison below.
        let rows: Vec<Vec<Value>> =
            cells.iter().map(|row| row.iter().map(|c| Value::Str(c.clone())).collect()).collect();
        let opts = ToonOptions { type_name: type_name.clone(), fields: fields.clone(), rows: rows.clone(), total_count, hints: vec![], max_cell_len: 200 };
        let rendered = render_toon(&opts);

        let parsed = parse(&rendered).expect("render_toon output must be parseable by the grammar");
        prop_assert_eq!(parsed.type_name, type_name);
        prop_assert_eq!(parsed.fields, fields);
        prop_assert_eq!(parsed.rows.len(), row_count);
        prop_assert_eq!(parsed.total_count, total_count);
        for (parsed_row, original_row) in parsed.rows.iter().zip(rows.iter()) {
            for (parsed_cell, original_value) in parsed_row.iter().zip(original_row.iter()) {
                // escape_value strips embedded \n/\r rather than preserving
                // them (TOON has no multi-line cell syntax), so the expected
                // value must be normalized the same way before comparing.
                let expected = value_to_string(original_value).replace(['\n', '\r'], "");
                prop_assert_eq!(parsed_cell, &expected);
            }
        }
    }
}
