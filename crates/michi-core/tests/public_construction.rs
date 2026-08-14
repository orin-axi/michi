//! AC-020: `StatusItem` is NOT `#[non_exhaustive]` -- struct-literal
//! construction from outside the crate must compile and succeed (contrast
//! `tests/ui_fail.rs`, where the same construction on `ContentBlock`/
//! `CallToolResult` must fail).
//! pipeline AC-008/AC-009: `PipelineStep`/`Pipeline` fields are directly
//! readable and writable from outside the module (neither is
//! `#[non_exhaustive]`).

use michi_core::pipeline::{Pipeline, PipelineStep, StepStatus};
use michi_core::{Health, KvValue, StatusItem};

#[test]
fn ac020_status_item_struct_literal_compiles_from_outside_the_crate() {
    let item = StatusItem { key: "index".to_string(), value: KvValue::Int(1), health: Some(Health::Ok) };
    assert_eq!(item.key, "index");
    assert_eq!(item.value, KvValue::Int(1));
    assert_eq!(item.health, Some(Health::Ok));
}

#[test]
fn ac008_ac009_pipeline_and_pipeline_step_fields_are_readable_and_writable_from_outside() {
    let mut step = PipelineStep { id: "s".to_string(), name: "n".to_string(), status: StepStatus::Pending };
    step.status = StepStatus::Completed;
    assert_eq!(step.id, "s");
    assert_eq!(step.name, "n");
    assert_eq!(step.status, StepStatus::Completed);

    let mut pipeline = Pipeline { id: "p".to_string(), steps: vec![step] };
    pipeline.id = "changed".to_string();
    assert_eq!(pipeline.id, "changed");
    assert_eq!(pipeline.steps.len(), 1);
}
