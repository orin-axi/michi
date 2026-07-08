use divan::Bencher;
use michi::toon::{render_toon, ToonOptions, Value};

fn main() {
    divan::main();
}

fn make_opts(n: usize) -> ToonOptions {
    ToonOptions {
        type_name: "issue".into(),
        fields: vec!["number".into(), "title".into(), "state".into()],
        rows: (0..n)
            .map(|i| {
                vec![Value::Int(i as i64), Value::Str(format!("Issue title number {i}")), Value::Str("open".into())]
            })
            .collect(),
        total_count: Some(1000),
        hints: vec!["Call get_issue with number=<number>".into()],
        ..Default::default()
    }
}

#[divan::bench(args = [1, 10, 100, 1000])]
fn render_n_rows(b: Bencher, n: usize) {
    let opts = make_opts(n);
    b.bench(|| render_toon(&opts));
}

#[divan::bench]
fn render_with_comma_escaping(b: Bencher) {
    let opts = ToonOptions {
        type_name: "item".into(),
        fields: vec!["name".into()],
        rows: (0..100).map(|i| vec![Value::Str(format!("Item {i}, with comma"))]).collect(),
        total_count: None,
        ..Default::default()
    };
    b.bench(|| render_toon(&opts));
}
