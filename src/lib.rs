#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]
#![warn(clippy::disallowed_types)]

//! # michi
//!
//! AXI response primitives for agent-ergonomic tools.
//!
//! `michi` facade crate re-exporting the underlying workspace crates:
//! - `michi-truncate`
//! - `michi-resilience`
//! - `michi-toon`
//! - `michi-core`

pub use michi_core::*;
pub use michi_resilience as resilience;
pub use michi_toon as toon;
pub use michi_truncate as truncate;

#[cfg(feature = "napi")]
pub mod napi;

/// Crate-level `Result` alias.
pub type Result<T> = std::result::Result<T, michi_core::Error>;
