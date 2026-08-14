# michi — Agent Instructions

## Commands

- `just test` — run all tests (Rust + Node)
- `just test-rust` — Rust tests only (cargo nextest)
- `just check` — fmt + clippy (`--all-targets`, matches the real pre-push gate) + deny + typos + markdown fmt
- `just build-node` — compile NAPI binary for local platform
- `just bench` — run divan benchmarks
- `just snapshots` — review insta snapshot diffs
- `just coverage` — generate lcov.info

## Non-negotiables

- No `unwrap()`/`expect()` in lib code (tests OK with message)
- No `unsafe` outside napi boundary
- `BTreeMap` for output maps, `HashMap` for internal state only
- Pre-allocate strings with `String::with_capacity`
- Truncation uses char boundaries (`floor_char_boundary`)
- Every `pub` item has a doc comment
- NAPI: `version = "3"`, `#[napi(catch_unwind)]` on every export
- Benchmarks: divan (not criterion)
- Tests: `cargo nextest run` (not `cargo test`)

## Philosophy

- Before adding a new primitive or judging whether work is "done," read `PRINCIPLES.md` — the inclusion checklist and process conventions this project runs on.

## Architecture

- `src/` — pure sync rendering library (default features: zero runtime deps)
- `packages/michi-node/` — thin NAPI cdylib shim, npm package name `@orin-axi/michi`
- Feature flags: `napi`, `serde` (opt-in Serialize/Deserialize + `toon::list()`), `schemars`, `miette`. Plan 2 (`pipeline`/`fuzzy`/`cache`/`cli` _execution_) lands as separate crates when built, never as features here — see `docs/spec/01-overview-and-setup.md`'s Cargo.toml section for the current split (`pipeline`'s data model already exists in `michi-core` today; only orchestration is deferred).
- See `docs/superpowers/specs/2026-07-03-michi-design.md` for decisions
- See `docs/spec/` for full module API reference

## Module guide

| Module | Purpose |
| --- | --- |
| `toon` | TOON list rendering — token-optimized agent list format |
| `kv` | Key-value single-item rendering |
| `hints` | `help[]` hint blocks |
| `truncate` | Token-safe content truncation |
| `empty` | Definitive empty state responses |
| `error` | Unified `michi::Error` type with agent rendering |
| `idempotency` | Partial-success reporting (`PartialSuccess`/`FailedOp`) — despite the name, idempotency keys and already-done detection live in `resilience` (`AlreadyDone`/`already_done()`), not here |
| `resilience` | Retry config, delay calculation, retry-after parsing |
| `status` | Health/status response rendering |
| `recovery` | Recovery hint blocks |
| `response` | `AgentResponse` builder — composes all primitives |
| `mcp` | MCP `CallToolResult` assembly — always compiled, no feature gate |
| `audience` | `Audience` (assistant/user) — shared by `mcp` and `response::render_for()` |
| `pipeline` | Pipeline pure data type + render (execution in Plan 2) |
| `telemetry` | No-op telemetry provider (`NoopProvider`) — zero-cost default, always compiled |
