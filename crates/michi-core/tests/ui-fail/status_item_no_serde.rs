fn main() {
    let item = michi_core::StatusItem { key: "k".to_string(), value: michi_core::KvValue::Int(1), health: None };
    let _ = serde_json::to_string(&item);
}
