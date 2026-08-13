#![deny(unsafe_code)]
#![warn(missing_docs)]

//! # michi-core
//!
//! Core AXI response types, AgentResponse builder, Audience routing, and structured diagnostics.

/// Audience routing (`assistant` vs `user`).
pub mod audience;
/// Definitive empty states.
pub mod empty;
/// Unified domain errors.
pub mod error;
/// Contextual usage hints (`help[]`).
pub mod hints;
/// Idempotency: partial-success state for multi-step operations.
pub mod idempotency;
/// Key-value single-item rendering (`key: value`).
pub mod kv;
/// MCP `CallToolResult` mapping.
pub mod mcp;
/// Pipeline step definitions and run state.
pub mod pipeline;
/// Structured recovery hints (`recovery[]`).
pub mod recovery;
/// `AgentResponse` builder.
pub mod response;
/// Health and status response rendering.
pub mod status;
/// No-op telemetry provider.
pub mod telemetry;

pub use audience::Audience;
pub use empty::{empty_state, empty_state_with_hints};
pub use error::{DomainError, Error, ErrorClass, ErrorCode, Sensitive};
pub use hints::{append_hints, render_hints, Hint};
pub use idempotency::{FailedOp, PartialSuccess};
pub use kv::{render_kv, KvItem, KvValue};
pub use mcp::{CallToolResult, ContentBlock};
pub use recovery::{append_recovery, render_recovery, RecoveryHint};
pub use response::{AgentResponse, OutputFormat};
pub use status::{Health, StatusItem, StatusResponse};

pub use michi_resilience::*;
pub use michi_truncate::*;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn ac033_crate_root_carries_deny_unsafe_and_warn_missing_docs() {
        let src = include_str!("lib.rs");
        assert!(src.contains("#![deny(unsafe_code)]"), "missing #![deny(unsafe_code)]");
        assert!(src.contains("#![warn(missing_docs)]"), "missing #![warn(missing_docs)]");
    }

    #[test]
    #[cfg(feature = "serde")]
    fn ac036_positive_half_types_serialize_under_serde_feature() {
        assert!(serde_json::to_string(&Audience::Assistant).is_ok());
        assert!(serde_json::to_string(&Hint::from("x")).is_ok());
        assert!(serde_json::to_string(&RecoveryHint::new("t")).is_ok());
        assert!(serde_json::to_string(&ContentBlock::new("t", vec![])).is_ok());
        assert!(serde_json::to_string(&CallToolResult::new(vec![], false, "{}")).is_ok());
    }

    #[test]
    fn test_auto_traits() {
        assert_send_sync_static::<Audience>();
        assert_send_sync_static::<DomainError>();
        assert_send_sync_static::<ErrorClass>();
        assert_send_sync_static::<ErrorCode>();
        assert_send_sync_static::<Hint>();
        assert_send_sync_static::<KvItem>();
        assert_send_sync_static::<KvValue>();
        assert_send_sync_static::<ContentBlock>();
        assert_send_sync_static::<CallToolResult>();
        assert_send_sync_static::<RecoveryHint>();
        assert_send_sync_static::<AgentResponse>();
        assert_send_sync_static::<OutputFormat>();
        assert_send_sync_static::<Health>();
        assert_send_sync_static::<StatusItem>();
        assert_send_sync_static::<StatusResponse>();
    }
}
