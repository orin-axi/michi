//! Thin cdylib shim exposing `michi`'s `napi` feature surface as the `michi`
//! npm package.
//!
//! The actual `#[napi]` export implementations live in `michi::napi`
//! (`src/napi.rs` in the workspace root crate) so they can be unit-tested
//! alongside the rest of the crate. This crate exists only because
//! `crate-type = ["cdylib"]` cannot coexist with a regular `[lib]` in the
//! same `Cargo.toml` as the main `michi` crate — see
//! `docs/superpowers/specs/2026-07-03-michi-design.md` §1.

pub use michi::napi::*;
