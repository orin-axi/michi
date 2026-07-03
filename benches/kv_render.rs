use divan::Bencher;
use michi::kv::{render_kv, KvItem, KvValue};

fn main() {
    divan::main();
}

#[divan::bench(args = [1, 5, 20])]
fn render_n_items(b: Bencher, n: usize) {
    let items: Vec<KvItem> =
        (0..n).map(|i| KvItem { key: format!("field_{i}"), value: KvValue::Str(format!("value_{i}")) }).collect();
    b.bench(|| render_kv(&items));
}
