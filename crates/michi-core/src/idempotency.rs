/// A single operation that failed within a larger multi-step operation.
#[derive(Debug, Clone, PartialEq)]
pub struct FailedOp {
    /// Identifier of the operation that failed.
    pub operation: String,
    /// Human-readable failure reason.
    pub reason: String,
    /// Optional structured recovery hint for this specific failure.
    pub recovery: Option<crate::recovery::RecoveryHint>,
}

/// Signals that an operation partially completed before a failure.
///
/// Use this when some steps of a multi-step operation succeeded — the agent
/// can resume from the checkpoint rather than retrying from scratch.
///
/// For a fully-completed prior call, see [`michi_resilience::AlreadyDone`]
/// and [`michi_resilience::already_done`].
#[derive(Debug, Clone, PartialEq)]
pub struct PartialSuccess {
    /// Identifiers of steps that completed successfully.
    pub completed: Vec<String>,
    /// Steps that failed, with reason and optional recovery hint.
    pub failed: Vec<FailedOp>,
    /// Identifiers of steps that were not attempted.
    pub skipped: Vec<String>,
}

impl PartialSuccess {
    /// Render as an agent-readable string: a summary line, one block per
    /// non-empty outcome category, then any per-op recovery hints folded into
    /// a trailing `help[]` block.
    ///
    /// Format:
    /// ```text
    /// partial_success: 2 completed, 1 failed, 1 skipped
    /// completed[2]:
    ///   create_issue
    ///   add_label
    /// failed[1]{operation,reason}:
    ///   assign_user,"User 'ghost' not found"
    /// skipped[1]:
    ///   notify_team
    /// help[1]:
    ///   assign_user: suggestedParams: { user: alice }
    /// ```
    ///
    /// Empty categories are omitted.
    #[must_use]
    pub fn render(&self) -> String {
        let mut out = String::with_capacity(
            64 + self.completed.iter().map(String::len).sum::<usize>()
                + self.failed.iter().map(|f| f.operation.len() + f.reason.len() + 60).sum::<usize>()
                + self.skipped.iter().map(String::len).sum::<usize>(),
        );
        out.push_str("partial_success: ");
        out.push_str(&self.completed.len().to_string());
        out.push_str(" completed, ");
        out.push_str(&self.failed.len().to_string());
        out.push_str(" failed, ");
        out.push_str(&self.skipped.len().to_string());
        out.push_str(" skipped\n");

        if !self.completed.is_empty() {
            out.push_str("completed[");
            out.push_str(&self.completed.len().to_string());
            out.push_str("]:\n");
            for op in &self.completed {
                out.push_str("  ");
                out.push_str(op);
                out.push('\n');
            }
        }

        if !self.failed.is_empty() {
            out.push_str("failed[");
            out.push_str(&self.failed.len().to_string());
            out.push_str("]{operation,reason}:\n");
            for f in &self.failed {
                out.push_str("  ");
                // operation: conditionally quoted (only when it contains commas/quotes/newlines)
                out.push_str(michi_toon::escape_value(&f.operation).as_ref());
                out.push(',');
                // reason: always quoted — it is human-readable prose and often contains quotes
                out.push_str(&michi_toon::escape_value_quoted(&f.reason));
                out.push('\n');
            }
        }

        if !self.skipped.is_empty() {
            out.push_str("skipped[");
            out.push_str(&self.skipped.len().to_string());
            out.push_str("]:\n");
            for op in &self.skipped {
                out.push_str("  ");
                out.push_str(op);
                out.push('\n');
            }
        }

        let recovery_hints: Vec<crate::recovery::RecoveryHint> =
            self.failed.iter().filter_map(|f| f.recovery.clone()).collect();
        if !recovery_hints.is_empty() {
            out.push_str("help[");
            out.push_str(&recovery_hints.len().to_string());
            out.push_str("]:\n");
            // Use inner append_recovery_lines (pub(crate)) to avoid the `recovery[N]:` header
            // that append_recovery would write — the section header here is `help[N]:`.
            crate::recovery::append_recovery_lines(&mut out, &recovery_hints);
        }

        out
    }

    /// `0` when all operations completed or were skipped; `1` when any failed.
    #[must_use]
    pub fn exit_code(&self) -> i32 {
        i32::from(!self.failed.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kv::KvValue;
    use crate::recovery::RecoveryHint;

    #[test]
    fn partial_success_full_example_matches_spec() {
        let ps = PartialSuccess {
            completed: vec!["create_issue".into(), "add_label".into()],
            failed: vec![FailedOp {
                operation: "assign_user".into(),
                reason: "User 'ghost' not found".into(),
                recovery: Some(RecoveryHint::new("assign_user").param("user", KvValue::Text("alice".into()))),
            }],
            skipped: vec!["notify_team".into()],
        };
        let out = ps.render();
        assert!(out.starts_with("partial_success: 2 completed, 1 failed, 1 skipped\n"), "got: {out}");
        assert!(out.contains("completed[2]:\n  create_issue\n  add_label\n"), "got: {out}");
        assert!(out.contains("failed[1]{operation,reason}:\n  assign_user,\"User 'ghost' not found\"\n"), "got: {out}");
        assert!(out.contains("skipped[1]:\n  notify_team\n"), "got: {out}");
        assert!(out.contains("help[1]:"), "got: {out}");
        assert!(out.contains("assign_user: suggestedParams: { user: alice }"), "got: {out}");
    }

    #[test]
    fn partial_success_empty_categories_omitted() {
        let ps = PartialSuccess { completed: vec!["a".into()], failed: vec![], skipped: vec![] };
        let out = ps.render();
        assert!(!out.contains("failed["), "empty failed category must be omitted, got: {out}");
        assert!(!out.contains("skipped["), "empty skipped category must be omitted, got: {out}");
    }

    #[test]
    fn partial_success_exit_code_zero_when_no_failures() {
        let ps = PartialSuccess { completed: vec!["a".into()], failed: vec![], skipped: vec!["b".into()] };
        assert_eq!(ps.exit_code(), 0);
    }

    #[test]
    fn partial_success_exit_code_one_when_any_failed() {
        let ps = PartialSuccess {
            completed: vec![],
            failed: vec![FailedOp { operation: "x".into(), reason: "y".into(), recovery: None }],
            skipped: vec![],
        };
        assert_eq!(ps.exit_code(), 1);
    }

    #[test]
    fn failed_op_without_recovery_produces_no_help_block() {
        let ps = PartialSuccess {
            completed: vec![],
            failed: vec![FailedOp { operation: "x".into(), reason: "y".into(), recovery: None }],
            skipped: vec![],
        };
        assert!(!ps.render().contains("help["));
    }

    #[test]
    fn operation_with_comma_is_escaped() {
        let ps = PartialSuccess {
            completed: vec![],
            failed: vec![FailedOp {
                operation: "create_issue, retry 2".into(),
                reason: "timeout".into(),
                recovery: None,
            }],
            skipped: vec![],
        };
        let out = ps.render();
        // operation contains comma — must be quoted to avoid breaking TOON column parsing
        assert!(out.contains(r#""create_issue, retry 2""#), "operation must be quoted, got: {out}");
    }
}
