use std::fmt::Write as _;

/// Status of an individual pipeline step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum StepStatus {
    /// Step completed successfully.
    Completed,
    /// Step was skipped.
    Skipped,
    /// Step failed.
    Failed,
    /// Step has not been attempted yet.
    Pending,
}

impl StepStatus {
    /// The short label string used in rendered output.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
            Self::Pending => "pending",
        }
    }
}

/// A pipeline step definition.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PipelineStep {
    /// Step identifier.
    ///
    /// michi does not validate uniqueness — callers are responsible for ensuring
    /// step IDs are distinct within a pipeline if they intend to reference steps
    /// by ID. Duplicate IDs render without error but produce ambiguous output.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Current status.
    pub status: StepStatus,
}

/// A pipeline run state.
#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct Pipeline {
    /// Pipeline identifier.
    pub id: String,
    /// All steps in declaration order.
    pub steps: Vec<PipelineStep>,
}

impl Pipeline {
    /// Render the pipeline state as a TOON-style list.
    #[must_use]
    pub fn render(&self) -> String {
        let n = self.steps.len();
        let capacity = 30 + n * 40;
        let mut out = String::with_capacity(capacity);

        out.push_str("step[");
        let _ = write!(out, "{n}");
        out.push_str("]{id,name,status}:\n");

        for step in &self.steps {
            out.push_str("  ");
            out.push_str(&michi_toon::escape_value(&step.id));
            out.push(',');
            out.push_str(&michi_toon::escape_value(&step.name));
            out.push(',');
            out.push_str(step.status.label());
            out.push('\n');
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_pipeline_steps() {
        let p = Pipeline {
            id: "my-pipeline".into(),
            steps: vec![
                PipelineStep { id: "fetch".into(), name: "Fetch Data".into(), status: StepStatus::Completed },
                PipelineStep { id: "upload".into(), name: "Upload".into(), status: StepStatus::Pending },
            ],
        };
        let out = p.render();
        assert_eq!(out, "step[2]{id,name,status}:\n  fetch,Fetch Data,completed\n  upload,Upload,pending\n");
    }
}
