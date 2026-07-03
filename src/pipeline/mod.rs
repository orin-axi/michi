#[cfg(feature = "pipeline")]
pub mod executor;

/// Status of an individual pipeline step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    /// Step completed successfully.
    Completed,
    /// Step was skipped (dependency failed or step was configured best-effort).
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

/// A pipeline step definition (pure data — no execution logic in Plan 1).
#[derive(Debug, Clone)]
pub struct PipelineStep {
    /// Unique step identifier.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Current status.
    pub status: StepStatus,
}

/// A pipeline run state (pure data, always compiled).
///
/// Renderable without the `pipeline` feature. The executor (Plan 2) produces
/// and updates this struct during a run.
#[derive(Debug, Clone, Default)]
pub struct Pipeline {
    /// Pipeline identifier (e.g. workflow name).
    pub id: String,
    /// All steps in declaration order.
    pub steps: Vec<PipelineStep>,
}

impl Pipeline {
    /// Render the pipeline state as a TOON-style list for agent consumption.
    ///
    /// Format:
    /// ```text
    /// step[2]{id,name,status}:
    ///   fetch-data,Fetch Data,completed
    ///   upload,Upload,pending
    /// ```
    #[must_use]
    pub fn render(&self) -> String {
        let n = self.steps.len();
        let capacity = 30 + n * 40;
        let mut out = String::with_capacity(capacity);

        out.push_str("step[");
        out.push_str(&n.to_string());
        out.push_str("]{id,name,status}:\n");

        for step in &self.steps {
            out.push_str("  ");
            out.push_str(&step.id);
            out.push(',');
            // Escape name if it contains a comma
            if step.name.contains(',') {
                out.push('"');
                out.push_str(&step.name);
                out.push('"');
            } else {
                out.push_str(&step.name);
            }
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

    #[test]
    fn empty_pipeline_renders_header_only() {
        let p = Pipeline::default();
        assert_eq!(p.render(), "step[0]{id,name,status}:\n");
    }

    #[test]
    fn step_name_with_comma_is_quoted() {
        let p = Pipeline {
            id: "p".into(),
            steps: vec![PipelineStep { id: "s".into(), name: "Parse, validate".into(), status: StepStatus::Completed }],
        };
        let out = p.render();
        assert!(out.contains(r#""Parse, validate""#));
    }

    #[test]
    fn step_status_labels() {
        assert_eq!(StepStatus::Completed.label(), "completed");
        assert_eq!(StepStatus::Skipped.label(), "skipped");
        assert_eq!(StepStatus::Failed.label(), "failed");
        assert_eq!(StepStatus::Pending.label(), "pending");
    }
}
