fn main() {
    let _: michi_core::Health = serde_json::from_str("\"ok\"").unwrap();
}
