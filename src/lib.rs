// ── Rust compiler lints ──────────────────────────────────────────────────────
// `deny`, not `forbid`: the napi module needs an inner `#![allow(unsafe_code)]`
// override for napi-derive's macro-generated FFI glue, which `forbid` (unlike
// `deny`) can never permit, even from macro expansion — see src/napi.rs.
#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(unreachable_pub)]
#![warn(unused_qualifications)]
#![warn(unused_lifetimes)]
// ── Clippy groups ────────────────────────────────────────────────────────────
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
// Pedantic allows — idiomatic patterns that pedantic flags as noise
#![allow(clippy::module_name_repetitions)] // ToonOptions in toon::, render_toon in toon:: — idiomatic
#![allow(clippy::single_char_lifetime_names)] // 'a, 'b are idiomatic
#![allow(clippy::wildcard_imports)] // use super::* in test modules is standard
#![allow(clippy::missing_errors_doc)] // add # Errors sections incrementally
#![allow(clippy::missing_panics_doc)] // add # Panics sections incrementally
#![allow(clippy::must_use_candidate)] // rendering fns — callers choose whether to use return value
#![allow(clippy::exhaustive_enums)] // intentional: users can match exhaustively
#![allow(clippy::exhaustive_structs)] // adding fields is not a breaking change for us
#![allow(clippy::doc_markdown)]
// too picky about backticks in comments
// ── Clippy restriction (safety + performance + style) ────────────────────────
#![warn(clippy::unwrap_used)]
#![warn(clippy::expect_used)]
#![warn(clippy::panic)]
#![warn(clippy::todo)]
#![warn(clippy::unimplemented)]
#![warn(clippy::indexing_slicing)]
#![warn(clippy::disallowed_macros)]
#![warn(clippy::implicit_clone)]
#![warn(clippy::inefficient_to_string)]
#![warn(clippy::use_self)]

//! # michi
//!
//! AXI response primitives for agent-ergonomic tools.
//!
//! `michi` (道) encodes seven of the ten AXI principles as typed, tested Rust:
//! TOON list rendering, key-value single-item rendering, contextual disclosure
//! (`help[]`), content truncation, definitive empty states, structured errors,
//! idempotency signals, retry delay primitives, and content-first status
//! responses.
//!
//! ## Feature flags
//!
//! | Feature | Adds |
//! |---|---|
//! | `pipeline` | `PipelineExecutor`, `CheckpointStore`, `OutputSink`, `CircuitBreaker` |
//! | `fuzzy` | `FuzzyMatcher`, `FuzzyResolver` |
//! | `cache` | Two-tier `Cache` (moka + disk) |
//! | `cli` | CLI surface adapters (indicatif, inquire) |
//! | `mcp` | MCP surface adapters |
//! | `napi` | NAPI exports (used by `packages/michi-node`) |
//! | `full` | All of the above except `napi` |
//!
//! Default features: none. A consumer with default features pulls in zero
//! async runtime dependencies.

/// Definitive empty-state responses: `type_name[0]{}:\ntotalCount: 0\n`.
pub mod empty;
/// Unified `Error` type with agent-renderable output and classification.
pub mod error;
/// `Hint` type and `help[N]:` block rendering.
pub mod hints;
/// Idempotency keys and already-done detection.
pub mod idempotency;
/// Key-value single-item rendering (`key: value\n` blocks).
pub mod kv;
/// NAPI export surface for the `michi` npm package (used by `packages/michi-node`).
#[cfg(feature = "napi")]
pub mod napi;
/// Pipeline step definitions and run-state rendering.
pub mod pipeline;
/// Structured recovery hints (`recovery[N]:` blocks).
pub mod recovery;
/// Retry configuration and delay calculation.
pub mod resilience;
/// `AgentResponse` builder — composes all michi primitives.
pub mod response;
/// Output sink abstractions (no-op placeholder; plan 2 adds real sinks).
pub mod sink;
/// Health and status response rendering.
pub mod status;
/// No-op telemetry provider (zero-cost default).
pub mod telemetry;
/// TOON list rendering — token-optimised agent list format.
pub mod toon;
/// Token-safe content truncation with agent-readable suffixes.
pub mod truncate;

// Re-export the most common types at the crate root for convenience.
pub use error::{DomainError, Error, ErrorClass, ErrorCode, Sensitive};
pub use hints::{append_hints, render_hints, Hint};
pub use response::{AgentResponse, OutputFormat};
pub use toon::{render_toon, ToonOptions, Value};

/// Crate-level `Result` alias.
pub type Result<T> = std::result::Result<T, Error>;
