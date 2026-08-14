fn main() {
    let s = michi_core::pipeline::StepStatus::Completed;
    match s {
        michi_core::pipeline::StepStatus::Completed => {}
        michi_core::pipeline::StepStatus::Skipped => {}
        michi_core::pipeline::StepStatus::Failed => {}
        michi_core::pipeline::StepStatus::Pending => {}
    }
}
