# Overview & Setup

michi is a Rust crate of pure agentic response primitives — the formatting and signaling
conventions that make tools ergonomic for LLM agents, regardless of protocol or language.

## What it is

AXI (Agent eXperience Interface) is a set of ten design principles for agent-ergonomic tooling
that treats token budget as a first-class constraint. Its core claim: "MCP vs. CLI" is the wrong
frame — the real question is which design principles make *any* interface effective for an LLM
agent. A well-designed interface following these principles measurably beats both naive CLIs and
MCP on task success rate, cost, duration, and turn count.

michi encodes the subset of AXI that's pure, language-agnostic computation — the parts worth one
canonical, tested implementation instead of ad hoc re-derivation in every tool.

**Seven of the ten principles, as typed Rust:**

| Principle | Module(s) |
|---|---|
| P1 — Token-Efficient Output | `toon`, `kv` |
| P3 — Content Truncation | `truncate` |
| P4 — Pre-Computed Aggregates | `ToonOptions::total_count`, `status` health summaries |
| P5 — Definitive Empty States | `empty` |
| P6 — Structured Errors & Exit Codes | `error`, `idempotency` |
| P8 — Content First | `status` |
| P9 — Contextual Disclosure | `hints`, `recovery` |

The other three stay out of scope on purpose. **P2 (Minimal Default Schemas)** is supported —
callers pass exactly the fields they want — but not enforced. **P7 (Ambient Context)** and
**P10 (Consistent Help)** are session-hook and CLI-framework concerns; they belong in the
consuming tool, not a rendering crate.

No protocol knowledge, no async runtime. Pure computation: data in, strings and types out. Rust
consumers take a direct crates.io or git dependency; TypeScript consumers reach it through the
`@orin-axi/michi` npm package.

## Why this exists

AXI's principles usually get implemented ad hoc — scattered TypeScript conventions in MCP
servers, implicit CLI patterns, per-tool string formatting that drifts over time. With michi:

- TOON has one canonical, tested implementation shared by every consumer
- `help[]` hints, `totalCount` formatting, truncation signals, and recovery shapes are defined
  once and can't drift between tools
- Any agent-facing Rust CLI imports the primitives directly
- Any TypeScript MCP server or CLI reaches them through the npm package

Non-agentic tools — build scripts, infrastructure CLIs — never touch michi. The package boundary
keeps agentic concepts out of code that doesn't need them.

## What's out of scope, deliberately

- **Display-format Markdown.** michi is the `audience: ["assistant"]` compact surface. Display
  rendering for `audience: ["user"]` stays in your MCP SDK or application layer.
- **Full MCP protocol knowledge.** No JSON-RPC, no tool registration, no server bootstrapping, no
  `outputSchema` validation. `AgentResponse::to_call_tool_result()` assembles the
  `content[]`/`isError`/`structuredContent` shape from an already-built response — see
  [04-mcp-and-napi.md](04-mcp-and-napi.md) — but nothing beyond that one assembly step.
- **CLI framework.** No argument parsing, no stdin/stdout handling.
- **HTTP client.** No auth, no request construction — just retry-delay primitives you plug into
  your own client.
- **Async runtime.** Zero tokio, zero async-std. Every function is sync and pure; you own the
  retry loop.
- **Full retry implementation.** michi gives you `RetryConfig`, `parse_retry_after()`,
  `next_retry_delay()`. The sleep-and-re-execute loop is yours.
- **Caching.** Use `moka` or equivalent.
- **Logging / telemetry.** Use `tracing` independently.
- **Schema validation.** Your concern, your type system.
- **MCP server bootstrapping.** `server.tool()`, tool discovery, deferred loading — stays in your
  SDK.

## Who consumes this, and how

```
Any Rust CLI binary
  Cargo.toml dep on michi (crates.io or git)
  Render --format toon via michi::render_toon()
  Full access to every module — no NAPI overhead

Any TypeScript CLI
  npm dep on @orin-axi/michi
  --format toon dispatch calls into the NAPI wrapper
  Same output; the NAPI boundary is transparent
  render_for()/renderFor() picks agent vs. human output

Any TypeScript MCP server
  npm dep on @orin-axi/michi
  Renders TOON for the audience:["assistant"] content block
  Assembles MCP content[] from the returned string
```

## Cargo.toml

```toml
[package]
name = "michi"
version = "0.1.0"
edition = "2021"
rust-version = "1.96"
description = "AXI response primitives for agent-ergonomic tools"
license = "AGPL-3.0-or-later"
repository = "https://github.com/orin-axi/michi"
keywords = ["axi", "agent", "mcp", "cli", "llm"]
categories = ["text-processing", "encoding", "development-tools"]

[features]
default = []
napi    = ["dep:napi", "dep:napi-derive", "dep:serde_json"]
serde   = ["dep:serde", "dep:serde_json"]

[dependencies]
thiserror  = "2"

[dependencies.napi]
version  = "3"
features = ["napi6", "serde-json"]
optional = true

[dependencies.napi-derive]
version  = "3"
optional = true

[dependencies.serde]
version  = "1"
features = ["derive"]
optional = true

[dependencies.serde_json]
version  = "1"
features = ["preserve_order"]
optional = true

[dev-dependencies]
divan    = "0.1"
proptest = "1"
insta    = { version = "1", features = ["yaml"] }

[[bench]]
name    = "toon_render"
harness = false

[[bench]]
name    = "kv_render"
harness = false
```

Default features add zero runtime dependencies. `serde_json` only enters the tree through `napi`
(typed `structuredContent`, wire-conformant MCP types) or `serde` (`Serialize`/`Deserialize` on
the core value types, plus `toon::list()`) — never both unconditionally, never with neither
enabled. `kv::KvValue` fills the role `serde_json::Value` would have for every consumer who
doesn't opt in, at zero dependency cost. `preserve_order` keeps `toon::list()`'s field order
matching each struct's declared order instead of alphabetizing it.

`pipeline`/`fuzzy`/`cache`/`cli` are not Cargo features of this crate at all — that async
execution layer (Plan 2) lands as genuinely separate crates when it's actually built, never again
as features gated on michi's own `Cargo.toml`. See [ARCHITECTURE.md](../../ARCHITECTURE.md) for
the crate-boundary layout and [06-decisions.md](06-decisions.md) for the decision rule behind that
split. `serde` isn't part of that Plan 2 set, despite historically sitting next to it in the
feature table — it gates `Serialize`/`Deserialize` on Plan 1's own types.

`napi-build` lives in `packages/michi-node/Cargo.toml`, not here — that's the cdylib crate the
actual napi-rs build step compiles.

## Crate layout

```
michi/
  Cargo.toml
  build.rs                      # napi-build (conditional on napi feature)
  src/
    lib.rs                      # public API, re-exports, crate-level docs
    toon/
      mod.rs                    # render_toon(), ToonOptions, Value
      escape.rs                 # comma/quote/null escaping
      render.rs                 # string assembly with pre-allocated capacity
    kv/
      mod.rs                    # render_kv(), KvItem, KvValue
    hints.rs                    # Hint, render_hints(), append_hints()
    truncate.rs                 # Truncated, truncate(), truncate_inline()
    empty.rs                    # empty_state(), empty_state_with_hints()
    error.rs                    # Error, ErrorCode, DomainError
    idempotency.rs              # IdempotencyKey, already_done(), PartialSuccess
    resilience/
      mod.rs                     # RetryConfig, parse_retry_after(), next_retry_delay()
                                  # CircuitBreaker/retry-wrapper (Plan 2) land in the future
                                  # pipeline crate, not here — see ARCHITECTURE.md
    status.rs                   # StatusItem, StatusResponse, Health
    audience.rs                 # Audience — always compiled
    mcp.rs                      # ContentBlock, CallToolResult — always compiled
    recovery.rs                 # RecoveryHint, render_recovery()
    response.rs                 # AgentResponse builder, OutputFormat
    napi.rs                     # #[napi] exports (napi feature only)
  benches/
    toon_render.rs
    kv_render.rs
  tests/
    toon_integration.rs
    kv_integration.rs
    snapshot_tests.rs           # insta snapshots
    proptest_toon.rs, proptest_truncate.rs, proptest_resilience.rs, proptest_mcp.rs
    toon_parser.rs, support/    # test-only TOON parser for round-trip property tests

packages/michi-node/            # NAPI wrapper (npm: @orin-axi/michi)
  Cargo.toml                    # napi feature, napi-rs build
  package.json                  # name: "@orin-axi/michi"
  index.js                      # platform binary loader + TS fallback
  index.d.ts                    # TypeScript types (auto-generated)
  src/
    lib.rs                      # #[napi] exports wrapping crate functions
  __test__/
    index.test.mjs              # node:test NAPI integration tests
```
