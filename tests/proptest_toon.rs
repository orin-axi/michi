use michi::toon::{render_toon, ToonOptions, Value};
use proptest::prelude::*;

mod toon_parser;
use toon_parser::{parse, value_to_string};

proptest! {
    #[test]
    fn render_toon_output_is_grammar_valid(
        type_name in "[a-z][a-z0-9_]{0,10}",
        field_count in 1usize..4,
        row_count in 0usize..5,
        cell in "[a-zA-Z0-9 ]{0,20}",
    ) {
        let fields: Vec<String> = (0..field_count).map(|i| format!("f{i}")).collect();
        let rows: Vec<Vec<Value>> = (0..row_count).map(|_| fields.iter().map(|_| Value::Str(cell.clone())).collect()).collect();
        let opts = ToonOptions { type_name: type_name.clone(), fields: fields.clone(), rows: rows.clone(), total_count: None, hints: vec![], max_cell_len: 200 };
        let rendered = render_toon(&opts);

        let parsed = parse(&rendered).expect("render_toon output must be parseable by the grammar");
        prop_assert_eq!(parsed.type_name, type_name);
        prop_assert_eq!(parsed.fields, fields);
        prop_assert_eq!(parsed.rows.len(), row_count);
        for (parsed_row, original_row) in parsed.rows.iter().zip(rows.iter()) {
            for (parsed_cell, original_value) in parsed_row.iter().zip(original_row.iter()) {
                prop_assert_eq!(parsed_cell, &value_to_string(original_value));
            }
        }
    }
}
