/// An opaque idempotency key derived from operation inputs.
///
/// Used to detect already-completed operations without re-executing them.
/// Callers derive the key from stable operation parameters, look it up in
/// their own store, and pass the lookup result — not the key itself — to
/// [`already_done`]: the key selects *which* record to check, and
/// `already_done` only needs to know what (if anything) that lookup found.
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

    /// Construct a key from an operation name and raw input bytes, hashed
    /// with FNV-1a for a stable, deterministic, low-collision key. Not
    /// cryptographic — idempotency keys need stability, not security. For
    /// maps/structs, serialize with sorted keys first (e.g. `BTreeMap`, not
    /// `HashMap`) so the same logical input always hashes the same way.
    #[must_use]
    pub fn from_hash(operation: &str, data: &[u8]) -> Self {
        Self(format!("{operation}:{:016x}", fnv1a_64(data)))
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

/// Check whether an operation has already completed.
///
/// Pass `stored` as `Some(result)` if a lookup in your own store by
/// [`IdempotencyKey`] found an entry; `None` if not. michi does not own any
/// persistence — the caller derives the key, retrieves and persists results
/// in their own store, and passes only the lookup outcome here.
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

/// Render an already-done response: a successful no-op, not an error (exit
/// code 0 is the caller's responsibility — this function only renders).
///
/// Format:
/// ```text
/// operation: create_issue
/// status:    already_done
/// summary:   Issue #42 already exists with identical fields
/// help[1]:
///   Call get_issue with number=42 to view it
/// ```
#[must_use]
pub fn render_already_done(operation: &str, summary: &str, hints: &[crate::hints::Hint]) -> String {
    let mut out = String::with_capacity(64 + operation.len() + summary.len() + hints.len() * 50);
    out.push_str("operation: ");
    out.push_str(operation);
    out.push_str("\nstatus:    already_done\nsummary:   ");
    out.push_str(summary);
    out.push('\n');
    crate::hints::append_hints(&mut out, hints);
    out
}

/// FNV-1a 64-bit hash. Fixed, versionless algorithm — unlike
/// `std::collections::hash_map::DefaultHasher`, whose algorithm Rust
/// explicitly does not guarantee stays the same across compiler versions.
fn fnv1a_64(data: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
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
    fn from_hash_is_deterministic() {
        let a = IdempotencyKey::from_hash("create_item", b"same input");
        let b = IdempotencyKey::from_hash("create_item", b"same input");
        assert_eq!(a, b);
    }

    #[test]
    fn from_hash_differs_by_operation() {
        let a = IdempotencyKey::from_hash("create_item", b"x");
        let b = IdempotencyKey::from_hash("delete_item", b"x");
        assert_ne!(a, b, "same data, different operation, must produce different keys");
    }

    #[test]
    fn from_hash_differs_by_data() {
        let a = IdempotencyKey::from_hash("create_item", b"x");
        let b = IdempotencyKey::from_hash("create_item", b"y");
        assert_ne!(a, b);
    }

    #[test]
    fn from_hash_produces_stable_known_value() {
        // Locks the exact FNV-1a output for a fixed input, so a future accidental
        // algorithm change is caught by a failing test, not silent key drift.
        let key = IdempotencyKey::from_hash("op", b"data");
        assert_eq!(key.as_str(), "op:855b556730a34a05");
    }

    #[test]
    fn from_hash_zero_pads_short_hex_digests() {
        // fnv1a_64(b"aa") == 0x089c4307b54596b7, which has a leading zero
        // nibble. Confirms `{:016x}` keeps the hash segment a fixed 16
        // characters instead of trimming it to 15, which would make key
        // width inconsistent across inputs.
        let key = IdempotencyKey::from_hash("op", b"aa");
        assert_eq!(key.as_str(), "op:089c4307b54596b7");
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

    #[test]
    fn render_already_done_matches_spec_format() {
        let out = render_already_done(
            "create_issue",
            "Issue #42 already exists with identical fields",
            &[crate::hints::Hint::new("Call get_issue with number=42 to view it")],
        );
        assert_eq!(
            out,
            "operation: create_issue\nstatus:    already_done\nsummary:   Issue #42 already exists with identical fields\nhelp[1]:\n  Call get_issue with number=42 to view it\n"
        );
    }

    #[test]
    fn render_already_done_no_hints_omits_help_block() {
        let out = render_already_done("noop", "nothing changed", &[]);
        assert!(!out.contains("help["));
    }
}
