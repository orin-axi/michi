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

pub mod empty;
pub mod error;
pub mod hints;
pub mod idempotency;
pub mod kv;
pub mod pipeline;
pub mod recovery;
pub mod resilience;
pub mod response;
pub mod sink;
pub mod status;
pub mod telemetry;
pub mod toon;
pub mod truncate;

// Re-export the most common types at the crate root for convenience.
pub use error::{Error, ErrorClass, Sensitive};
pub use hints::{append_hints, render_hints, Hint};
pub use response::{AgentResponse, OutputFormat};
pub use toon::{render_toon, ToonOptions, Value};

/// Crate-level `Result` alias.
pub type Result<T> = std::result::Result<T, Error>;
