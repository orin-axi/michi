//! AC-027/AC-029: `#[non_exhaustive]` on `ContentBlock`/`CallToolResult`
//! prevents struct-literal construction from outside this crate.
//! AC-036: `Health` has no `Serialize` impl even with the `serde` feature on.

#[test]
fn non_exhaustive_and_no_serde_types_reject_the_forbidden_construction() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui-fail/content_block_struct_literal.rs");
    t.compile_fail("tests/ui-fail/call_tool_result_struct_literal.rs");
    #[cfg(feature = "serde")]
    {
        t.compile_fail("tests/ui-fail/health_no_serde.rs");
        t.compile_fail("tests/ui-fail/status_item_no_serde.rs");
        t.compile_fail("tests/ui-fail/status_response_no_serde.rs");
        t.compile_fail("tests/ui-fail/health_no_deserialize.rs");
        t.compile_fail("tests/ui-fail/status_item_no_deserialize.rs");
        t.compile_fail("tests/ui-fail/status_response_no_deserialize.rs");
    }
}
