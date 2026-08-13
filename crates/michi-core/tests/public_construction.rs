//! AC-020: `StatusItem` is NOT `#[non_exhaustive]` -- struct-literal
//! construction from outside the crate must compile and succeed (contrast
//! `tests/ui_fail.rs`, where the same construction on `ContentBlock`/
//! `CallToolResult` must fail).

use michi_core::{Health, KvValue, StatusItem};

#[test]
fn ac020_status_item_struct_literal_compiles_from_outside_the_crate() {
    let item = StatusItem { key: "index".to_string(), value: KvValue::Int(1), health: Some(Health::Ok) };
    assert_eq!(item.key, "index");
    assert_eq!(item.value, KvValue::Int(1));
    assert_eq!(item.health, Some(Health::Ok));
}
