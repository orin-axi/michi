/// An opaque idempotency key derived from operation inputs.
///
/// Used to detect already-completed operations without re-executing them.
/// Callers derive the key from stable operation parameters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    /// Create an idempotency key from any string-like value.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The raw key string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for IdempotencyKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for IdempotencyKey {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

/// Result of an idempotency check.
#[derive(Debug, Clone, PartialEq)]
pub enum AlreadyDone {
    /// The operation completed in a previous call.
    Yes {
        /// The previously stored result.
        result: String,
    },
    /// The operation has not been seen before — proceed with execution.
    No,
}

/// Check whether an operation identified by `key` has already completed.
///
/// Pass `stored` as `Some(result)` if your store contains an entry for this
/// key; `None` if not. michi does not own any persistence — the caller
/// retrieves and persists results in their own store.
///
/// Returns [`AlreadyDone::Yes`] when `stored` is `Some`, [`AlreadyDone::No`]
/// otherwise.
#[must_use]
pub fn already_done(stored: Option<String>) -> AlreadyDone {
    match stored {
        Some(result) => AlreadyDone::Yes { result },
        None => AlreadyDone::No,
    }
}

/// Signals that an operation partially completed before a failure.
///
/// Use this when some steps of a multi-step operation succeeded — the agent
/// can resume from the checkpoint rather than retrying from scratch.
#[derive(Debug, Clone, PartialEq)]
pub struct PartialSuccess {
    /// Identifiers of steps that completed successfully.
    pub completed: Vec<String>,
    /// Identifiers of steps that were not attempted.
    pub remaining: Vec<String>,
    /// Human-readable reason for the partial completion.
    pub reason: String,
}

impl PartialSuccess {
    /// Render this partial success as an agent-readable string.
    #[must_use]
    pub fn render(&self) -> String {
        format!("partial: {} completed, {} remaining — {}", self.completed.len(), self.remaining.len(), self.reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn already_done_with_stored_result() {
        let result = already_done(Some("cached output".into()));
        assert_eq!(result, AlreadyDone::Yes { result: "cached output".into() });
    }

    #[test]
    fn already_done_with_none() {
        assert_eq!(already_done(None), AlreadyDone::No);
    }

    #[test]
    fn idempotency_key_equality() {
        let a = IdempotencyKey::new("key-1");
        let b: IdempotencyKey = "key-1".into();
        assert_eq!(a, b);
    }

    #[test]
    fn idempotency_key_from_string() {
        let k: IdempotencyKey = String::from("k").into();
        assert_eq!(k.as_str(), "k");
    }

    #[test]
    fn partial_success_renders() {
        let ps = PartialSuccess {
            completed: vec!["step-a".into(), "step-b".into()],
            remaining: vec!["step-c".into()],
            reason: "rate limit hit".into(),
        };
        let out = ps.render();
        assert!(out.contains("2 completed"));
        assert!(out.contains("1 remaining"));
        assert!(out.contains("rate limit hit"));
    }
}
