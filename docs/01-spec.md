# michi — Specification
> orin-axi · Draft · June 2026

---

## Overview

`michi` is a Rust crate of pure agentic response primitives — the
conventions that make tools ergonomic for LLMs regardless of protocol or
language. It is general-purpose: useful to anyone building AXI-compliant
tools, not tied to any specific stack.

AXI (Agent eXperience Interface) is a set of ten design principles for
agent-ergonomic tooling that treats token budget as a first-class constraint.
Its thesis is that the "MCP vs. CLI" debate is the wrong frame: the real
question is which design principles make *any* interface effective for an LLM
agent. A well-designed interface following these principles measurably
outperforms both naive CLIs and MCP on task success rate, cost, duration, and
number of turns. `michi` encodes the subset of those principles that are pure,
language-agnostic computation — the parts that benefit from a single canonical,
tested implementation rather than ad-hoc re-derivation in every tool.

It encodes **seven of the ten AXI principles** as typed, tested Rust: TOON
list rendering, key-value single-item rendering, contextual disclosure
(`help[]`), pre-computed aggregates (`totalCount`), content truncation,
definitive empty states, structured errors with exit codes, idempotency
signals, retry delay primitives, and content-first status responses.

| AXI principle | michi module(s) |
|---|---|
| **P1 — Token-Efficient Output** | `toon`, `kv` |
| **P3 — Content Truncation** | `truncate` |
| **P4 — Pre-Computed Aggregates** | `ToonOptions::total_count` (the `totalCount:` line), `status` health summaries |
| **P5 — Definitive Empty States** | `empty` |
| **P6 — Structured Errors & Exit Codes** | `error`, `idempotency` |
| **P8 — Content First** | `status` |
| **P9 — Contextual Disclosure** | `hints`, `recovery` |

The three remaining principles are deliberately out of scope: **P2 — Minimal
Default Schemas** is supported (callers pass exactly the fields they want) but
not enforced; **P7 — Ambient Context** and **P10 — Consistent Help** are
session-hook and CLI-framework concerns that live in the consuming tool, not in
a pure rendering crate.

The crate has no protocol knowledge (no MCP, no HTTP, no CLI framework) and
no async runtime dependency. It is pure computation: data in, strings and types
out. TypeScript consumers reach it via the NAPI npm wrapper `michi`. Rust
consumers take a direct crates.io or git dep.

---

## Motivation

AXI's ten design principles describe how to build agent-ergonomic interfaces,
but the principles themselves are typically implemented ad-hoc — scattered as
TypeScript conventions in MCP servers, implicit CLI output patterns, and
per-tool string formatting that drifts over time. When `michi` exists:

- The TOON format has a canonical, tested Rust implementation shared by both
  MCP and CLI consumers
- `help[]` hint construction, `totalCount` formatting, truncation signals, and
  recovery hint shapes are defined once and cannot drift between consumers
- Any agent-facing Rust CLI can import the same primitives directly
- Any TypeScript MCP server or CLI can reach them via the NAPI npm package

A non-agentic tool (infrastructure CLIs, build scripts) uses its own output
conventions and never encounters michi. The package boundary enforces the
separation — agentic concepts do not leak into infrastructure.

---

## Non-goals

- **Display-format Markdown** — research-grounded LLM display formatters
  (fields, sections, tables) are the caller's concern. michi is the
  `audience: ["assistant"]` compact surface; display rendering for the
  `audience: ["user"]` surface stays in your MCP SDK or application layer.
- **MCP protocol knowledge** — no `content[]`, no `outputSchema`, no
  `structuredContent`. Those are assembled by the calling MCP framework.
- **CLI framework** — no argument parsing, no stdin/stdout handling. That
  belongs to the caller's CLI framework of choice.
- **HTTP client** — no auth handling, no request construction. michi
  provides retry delay primitives that a caller plugs into their own HTTP
  client.
- **Async runtime** — zero tokio, zero async-std. All functions are sync
  and pure. Callers own retry loops and timeout semantics.
- **Full retry implementation** — michi provides `RetryConfig`,
  `parse_retry_after()`, and `next_retry_delay()`. The actual retry loop
  (sleep + re-execute) stays in the caller.
- **Caching** — callers should use `moka` or equivalent. michi does not
  include a cache.
- **Logging / telemetry** — Rust consumers use the `tracing` crate
  independently.
- **Schema validation** — input validation is the caller's concern.
- **MCP server bootstrapping** — `server.tool()`, tool discovery, deferred
  loading. Protocol-specific, stays in your MCP SDK.

---

## Consumer map

```
Any Rust CLI binary
  Cargo.toml dep on michi (crates.io or git)
  Render --format toon via michi::render_toon()
  Full access to all modules — no NAPI overhead

Any TypeScript CLI
  npm dep on michi
  --format toon dispatch calls into NAPI wrapper
  Same output; NAPI boundary is transparent

Any TypeScript MCP server
  npm dep on michi
  Renders TOON for audience:["assistant"] content block
  Assembles MCP content[] from the returned string
```

---

## Cargo.toml

> Deviations from earlier drafts of this section are tracked in
> `docs/superpowers/plans/2026-07-08-spec-parity.md`'s Design Decisions table.
> This section shows the actual, shipped Plan 1 (pure-primitives) surface —
> the workspace `Cargo.toml` also carries a `pipeline`/`fuzzy`/`cache`/`cli`/
> `mcp`/`full` feature set for the execution layer (Plan 2), which is out of
> scope for this spec.

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
napi    = ["dep:napi", "dep:napi-derive"]
cli     = []  # reserved: terminal-width-aware rendering, colour support

[dependencies]
thiserror  = "2"

[dependencies.napi]
version  = "3"
features = ["napi6"]
optional = true

[dependencies.napi-derive]
version  = "3"
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

`serde`/`serde_json` are deliberately absent — an earlier draft listed them as
unconditional dependencies, which an adversarial review found unused outside
NAPI-boundary conversions and removed, restoring the "zero deps by default"
guarantee this crate promises. `kv::KvValue` (typed scalar enum) fills the
same role `serde_json::Value` would have, at zero dependency cost. Benchmarks
use `divan`, not `criterion`, per this crate's own non-negotiables.

`napi-build` is not a dependency of *this* crate's `Cargo.toml` — the actual
napi-rs build step lives in `packages/michi-node/Cargo.toml`
(`[build-dependencies] napi-build = "2"`), since that's the cdylib crate napi
build tooling actually compiles.

The `napi = { features = ["napi6"] }` selection is deliberate — see the NAPI
section for the full rationale (napi-rs v3 removed the Docker cross-compile
requirement v2 had, so this crate moved off `napi4`/v2 as soon as v3 shipped).

---

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
    resilience.rs               # RetryConfig, parse_retry_after(), next_retry_delay()
    status.rs                   # StatusItem, StatusResponse, Health
    recovery.rs                 # RecoveryHint, render_recovery()
    response.rs                 # AgentResponse builder, OutputFormat
    napi.rs                     # #[napi] exports (napi feature only)
  benches/
    toon_render.rs
    kv_render.rs
  tests/
    toon_integration.rs
    kv_integration.rs
    resilience_integration.rs
    idempotency_integration.rs
    snapshot_tests.rs           # insta snapshots

packages/michi-node/            # NAPI wrapper (npm: michi)
  Cargo.toml                    # napi feature, napi-rs build
  package.json                  # name: "michi"
  index.js                      # platform binary loader + TS fallback
  index.d.ts                    # TypeScript types (auto-generated)
  src/
    lib.rs                      # #[napi] exports wrapping crate functions
  __test__/
    index.test.mjs              # node:test NAPI integration tests
```

---

## TOON format specification

TOON (Token-Optimized Object Notation) is the agent-facing list format.
It front-loads structure (type, count, field names) so the LLM has full
context before reading any row, then encodes rows as compact comma-separated
values. Field names appear once in the header rather than once per row.

This is the canonical implementation of **AXI Principle 1 (Token-Efficient
Output)**: every token in a response permanently consumes context-window
budget, and on multi-turn tasks that cost compounds across turns. TOON omits
the braces, quotes, and repeated keys that JSON spends on structure the LLM can
already infer positionally.

**Why TOON for lists?**
Standard JSON repeats field names on every item. Markdown key-value format
repeats field names per item, which is ideal for single items and small sets
but expensive at scale. TOON trades per-item field repetition for a one-time
header, saving proportionally more tokens as N grows. For lists of 5+ items,
TOON is the better token budget choice. For single items or small metadata
blocks, key-value format remains preferred — see `kv::render_kv()`.

AXI's benchmark shows this class of output optimization delivers approximately
40% token reduction over equivalent JSON for list data.

### Grammar

```
document     ::= type_header NEWLINE row+ totalcount? help_block?
type_header  ::= type_name "[" count "]" "{" field_list "}" ":"
type_name    ::= [a-z_][a-z0-9_]*                 (snake_case)
count        ::= [0-9]+                            (items in this response)
field_list   ::= field_name ("," field_name)*
field_name   ::= [a-z_][a-z0-9_]*
row          ::= "  " value ("," value)* NEWLINE   (2-space indent)
value        ::= scalar | quoted
scalar       ::= [^,\n"]*
quoted       ::= '"' ( [^"\\] | "\\" . )* '"'     (for values with commas)
totalcount   ::= "totalCount: " [0-9]+ NEWLINE     (total available, may exceed count)
help_block   ::= "help[" [0-9]+ "]:" NEWLINE hint+
hint         ::= "  " [^\n]+ NEWLINE
```

### Examples

**List response:**
```
issues[3]{number,title,state}:
  42,Fix login redirect,open
  43,Add dark mode,open
  44,"Update deps, bump major",closed
totalCount: 47
help[2]:
  Call get_issue with number=<number> for full detail
  Call list_issues with state=open to filter
```

**Truncated field value:**
```
components[2]{name,description,tokens}:
  Button,"Primary action element (148 chars truncated — use full=true)",12
  Icon,"Scalable vector icon (203 chars truncated — use full=true)",8
totalCount: 84
help[1]:
  Call get_component with name=<name> and full=true for complete description
```

**Empty state:**
```
issues[0]{}:
totalCount: 0
help[1]:
  Try list_issues with a broader filter
```

**Single recovery hint (no list):**
```
help[1]:
  Retry create_item with suggestedParams: { project: "PROJ", type: "Task" }
```

### Escaping rules

- Values containing commas MUST be quoted
- Values containing double-quote characters use backslash escape: `\"`
- Values containing newlines MUST be truncated before rendering — TOON
  does not support multi-line cell values
- Null/absent values render as empty scalar: `val1,,val3`
- Boolean values: `true` / `false` literals
- Numeric values: rendered without quotes

---

## Rust public API

### Crate root (`src/lib.rs`)

```rust
pub mod empty;
pub mod error;
pub mod hints;
pub mod idempotency;
pub mod kv;
pub mod recovery;
pub mod resilience;
pub mod response;
pub mod status;
pub mod toon;
pub mod truncate;

// Top-level re-exports for the common path
pub use empty::empty_state;
pub use error::{DomainError, Error, ErrorClass, ErrorCode, Sensitive};
pub use hints::{append_hints, render_hints, Hint};
pub use idempotency::{already_done, render_already_done, AlreadyDone, FailedOp, IdempotencyKey, PartialSuccess};
pub use kv::render_kv;
pub use recovery::RecoveryHint;
pub use resilience::{next_retry_delay, parse_retry_after, RetryConfig};
pub use response::{AgentResponse, OutputFormat};
pub use status::StatusResponse;
pub use toon::{render_toon, ToonOptions, Value};
pub use truncate::{truncate, truncate_inline, Truncated};
```

`error::AxiError` in an earlier draft is `error::{DomainError, Error, ...}`
today — see the `error` section for the rename/split rationale. The
re-export list also grew several items an earlier draft didn't list at all
(`append_hints`/`render_hints`, `render_already_done`/`AlreadyDone`/
`IdempotencyKey`, `ToonOptions`/`Value`, `Truncated`) as those types and
functions were built out; none of these are behavioral deviations, just an
incomplete original list.

The actual crate additionally has `pipeline`, `sink`, `telemetry`, and (behind
the `napi` feature) `napi` modules, plus a `pipeline`/`fuzzy`/`cache`/`cli`/
`mcp`/`full` feature set — the execution layer referenced in `CLAUDE.md`'s
module guide as "Plan 2." That surface is out of scope for this spec, which
documents only the pure-primitives (Plan 1) API above.

---

### `toon` — list rendering for N uniform-schema items

TOON is the agent-facing format for lists (**AXI P1**). It front-loads
structure (type, count, field names) and encodes rows as compact
comma-separated values. Field names appear once in the header rather than once
per row. Preferred over key-value format for lists of 5+ items where token
savings compound; for single items, use `kv::render_kv()` instead.

**Implementation note:** `render_toon()` pre-allocates output capacity via
`String::with_capacity` estimated from `items.len() × avg_row_estimate` (see
the supplement for the exact heuristic). This is the hot path — no intermediate
allocations. The `escape.rs` submodule handles comma/quote escaping inline
without heap allocation for the common case (no special characters).

An earlier draft of this section split `render_toon()`'s arguments across
five positional parameters plus a `ToonOptions` bag holding only
`max_cell_len`/`total_count`. The shipped signature instead folds every
input — including `type_name`/`fields`/`rows`/`hints` — into `ToonOptions`
itself, taken by reference:

```rust
pub struct ToonOptions {
    /// Snake_case type name, e.g. "issue", "component".
    pub type_name: String,
    /// Ordered field names for the header.
    pub fields: Vec<String>,
    /// Rows, each a Vec of values parallel to `fields`.
    pub rows: Vec<Vec<Value>>,
    /// Total items available. May exceed rows rendered.
    /// Emitted as "totalCount: N" line when Some. This is the canonical
    /// AXI P4 (pre-computed aggregate) — it tells the agent how many items
    /// exist without forcing a follow-up call.
    pub total_count: Option<usize>,
    /// Agent-facing usage hints. Emitted as `help[N]:` block when non-empty.
    pub hints: Vec<Hint>,
    /// Max cell value length before inline truncation.
    /// Appends "(N chars truncated — use full=true)" when exceeded.
    /// Default: 200
    pub max_cell_len: usize,
}

impl Default for ToonOptions { ... } // max_cell_len: 200

/// Render a TOON document to a string.
///
/// # Panics
/// In debug builds, panics if any row's length doesn't match `fields.len()`.
/// Release builds render the mismatched row as-is — a development-time
/// correctness signal, not an input-validation guarantee (an earlier draft
/// said release builds "silently skip" a mismatched row; the shipped
/// function has never indexed by field position, so there is nothing to
/// skip — a mismatched row already renders every value it has).
///
/// Returns empty-state TOON when `rows` is empty.
pub fn render_toon(opts: &ToonOptions) -> String

pub enum Value {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl From<&str>           for Value { ... }
impl From<String>         for Value { ... }
impl From<i64>            for Value { ... }
impl From<f64>            for Value { ... }
impl From<bool>           for Value { ... }
impl From<Option<String>> for Value { ... }
```

`From<f64>` maps to `Value::Float`; all five scalar variants have a `From`
conversion so callers can build rows with `.into()` uniformly.

---

### `kv` — single-item key-value rendering

Markdown key-value format for single items and small mixed-type metadata
sets. Benchmarks of LLM format retrieval accuracy show this format performs
strongly for single-item or small-N data where field name repetition is
acceptable. Column width is determined by the longest key.

```rust
pub struct KvItem {
    pub key:   String,
    pub value: KvValue,
}

pub enum KvValue {
    Text(String),
    Int(i64),
    Float(f64, u8),    // value, decimal places
    Bool(bool),
    Duration(std::time::Duration),
    Missing,           // renders as "—"
}

/// Render key-value pairs with aligned columns.
/// Appends totalCount and help[] block when provided.
pub fn render_kv(
    items: &[KvItem],
    total_count: Option<usize>,
    hints: &[Hint],
) -> String
```

Output example:
```
name:         Button
variant:      primary
tokens:       12
description:  Primary action element
totalCount: 1
help[1]:
  Call list_components to see related
```

---

### `hints` — contextual disclosure blocks

Implements **AXI Principle 9 (Contextual Disclosure)**: after every output,
append a `help[]` section suggesting logical next steps as concrete,
parameterized command templates. Carry forward fixed disambiguating flags from
the current context, but leave runtime values as named placeholders (`<id>`)
rather than guessing them. This eliminates the discovery round trip — the agent
sees the next logical actions in the output it just received instead of having
to guess subcommands or issue a separate `--help` call.

```rust
/// A single next-step hint appended after tool output.
/// Should be concrete: include tool name and parameter template.
///
/// Good:  "Call get_issue with number=<number> for full detail"
/// Avoid: "Use another tool for more information"
pub struct Hint(pub String);

impl Hint {
    pub fn new(text: impl Into<String>) -> Self
}

/// Render a standalone help[] block.
/// Returns empty string when hints is empty — callers can
/// safely append the result without a guard.
pub fn render_hints(hints: &[Hint]) -> String

/// Append a help[] block to an existing body string.
/// Returns body unchanged when hints is empty.
pub fn append_hints(body: &str, hints: &[Hint]) -> String
```

---

### `truncate` — size-bounded output with escape hatches

Implements **AXI Principle 3 (Content Truncation)**: large text fields are
capped to a configurable character limit with a size hint appended that
explains the truncation and provides an escape hatch (`use full=true`). A
single verbose field can otherwise consume thousands of tokens, exhausting the
context budget before the agent has read the rest of a list; truncation
preserves enough signal for most operations while reserving budget for
subsequent turns.

```rust
pub struct Truncated {
    pub content:      String,
    pub truncated:    bool,
    pub original_len: usize,
    /// "(N chars truncated — use full=true)" — None when not truncated
    pub signal:       Option<String>,
}

/// Truncate content to at most `limit` chars, respecting char boundaries.
/// Never splits a multi-byte UTF-8 sequence.
pub fn truncate(content: &str, limit: usize) -> Truncated

/// Truncate and append the signal inline.
/// Returns original string when it fits within limit.
pub fn truncate_inline(content: &str, limit: usize) -> String
```

---

### `empty` — definitive zero states

Implements **AXI Principle 5 (Definitive Empty States)**: when a query returns
no results, emit an explicit zero-result message rather than silent empty
output. Agents cannot reliably distinguish "no results" from "command failed
silently"; an explicit `[0]` count plus `totalCount: 0` removes the ambiguity
that otherwise drives retry loops.

```rust
/// Render an explicit empty-state TOON response.
/// Callers MUST use this — never return silent empty output.
///
/// Produces:
///   type_name[0]{}:
///   totalCount: 0
pub fn empty_state(type_name: &str) -> String

/// Empty state with contextual hints appended.
pub fn empty_state_with_hints(type_name: &str, hints: &[Hint]) -> String
```

---

### `error` — structured errors with exit codes

Implements **AXI Principle 6 (Structured Errors and Exit Codes)**: errors are
written to **stdout** (not stderr), carry a clean exit code (`0` success, `1`
error), carry recovery hints, and never block on interactive prompts. Agents
cannot answer `Are you sure? [y/n]`, may not capture stderr at all, and cannot
reliably parse freeform error text. `DomainError` encodes the structured
contract as a type.

The HTTP status → `ErrorCode` mapping is NOT in michi — callers produce an
`ErrorCode` after interpreting whatever failure occurred. This keeps the
module free of HTTP knowledge.

An earlier draft of this section named this type `AxiError` and had it be
the crate's whole error type. The actual, shipped shape splits this in two:
`DomainError` (below) is the pure data/render type this section describes;
`Error` is a `thiserror`-derived enum with a `Domain(DomainError)` variant
alongside separate, always-compiled `InvalidInput(String)`/`NotFound(String)`
variants and (behind the `pipeline` feature) execution-layer variants like
`Http`/`Timeout`/`StepFailed` that need `#[source]`-chaining, which a single
struct can't express. `Error::render()`/`Error::class()`/`Error::exit_code()`
delegate to `DomainError`'s when the variant is `Domain`.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    // Non-retryable — agent should not retry without changing params
    InvalidInput,    // bad parameters
    NotFound,        // resource absent
    Unauthorized,    // auth failure
    Forbidden,       // permission denied
    Conflict,        // resource state mismatch

    // Retryable — agent may retry same call
    RateLimited,     // 429 — check retry_after
    Unavailable,     // 503
    Timeout,         // request timed out
    ExternalFailure, // downstream error
}

impl ErrorCode {
    /// The snake_case label used in rendered output.
    pub fn label(&self) -> &'static str
    /// Whether this code is conventionally retryable, absent an explicit
    /// override via `DomainError::retryable`.
    pub fn is_retryable_by_default(&self) -> bool
}

/// A domain-level error: a classified code, message, and everything needed
/// to render an agent-actionable response.
#[derive(Debug, Clone)]
pub struct DomainError {
    pub code:        ErrorCode,
    pub message:     String,
    pub hints:       Vec<Hint>,
    pub recovery:    Option<RecoveryHint>,
    pub retryable:   bool,
    pub retry_after: Option<std::time::Duration>,
}

impl DomainError {
    /// `retryable` defaults from `code.is_retryable_by_default()`.
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self
    pub fn hint(mut self, hint: impl Into<String>) -> Self
    pub fn recovery(mut self, r: RecoveryHint) -> Self
    /// Override the default retryability for this specific error instance.
    pub fn retryable(mut self, retryable: bool) -> Self
    pub fn retry_after(mut self, d: std::time::Duration) -> Self

    /// Render to stdout-ready string with hints attached.
    pub fn render(&self) -> String

    /// AXI exit codes: 0 = success/already_done, 1 = all other errors.
    /// Callers should not invent additional codes. Always `1` today — michi
    /// has no error path that maps to a nonzero-but-not-1 exit code.
    pub fn exit_code(&self) -> i32
}

/// The unified error type for the michi crate. Carries both agent-renderable
/// information (`Error::render()`) and machine-readable classification
/// (`Error::class() -> ErrorClass`). Execution-layer variants (`Http`,
/// `Timeout`, etc.) exist only when the `pipeline` feature is enabled.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    InvalidInput(String),
    NotFound(String),
    Domain(DomainError),
    // ...execution-layer variants behind `pipeline` — out of scope here.
}
```

`ErrorCode` is `Copy + Eq` so callers can match and compare it freely, and
renders as snake_case. `DomainError::render()` produces a KV-shaped block on
stdout, e.g. for `DomainError::new(ErrorCode::NotFound, "Issue #9999 does not
exist in this repository").hint("Call list_issues to see available numbers")`:

```
error: not_found
message: Issue #9999 does not exist in this repository
exit_code: 1
help[1]:
  Call list_issues to see available numbers
```

---

### `idempotency` — already-done signals and partial success

Provides types and rendering for idempotency checks, already-done detection,
and partial-success reporting. This is the mutation-safety half of **AXI P6**:
write operations must be idempotent so an agent can confidently retry a
transient failure without duplicating records. The caller owns the persistence
layer that tracks what has been done; michi provides the rendering contract.

An earlier draft of this section modeled `already_done()` as a single
function that both checks *and* renders. The shipped design splits these
into two independent, unrelated-by-name functions, because michi owns no
persistence layer and therefore cannot itself decide whether an operation
was already done — only the caller's store can:

```rust
/// A canonical idempotency key.
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    /// Construct from any string-like value. Unlike an earlier draft's
    /// `new(operation, stable_input)`, this takes a single already-combined
    /// key string — callers that want an operation-name-plus-input key build
    /// it themselves (e.g. `format!("{operation}:{stable_input}")`) before
    /// calling `new`, or use `from_hash` below.
    pub fn new(s: impl Into<String>) -> Self
    /// Construct from an operation name and raw input bytes, hashed with
    /// FNV-1a (not SHA-256, per the zero-dep rationale — see
    /// `docs/superpowers/plans/2026-07-08-spec-parity.md`'s Design Decisions
    /// table: idempotency keys need stability and low collision, not
    /// cryptographic security, and `sha2` is gated behind the `cache`
    /// feature only).
    pub fn from_hash(operation: &str, data: &[u8]) -> Self
    pub fn as_str(&self) -> &str
}

/// Result of an idempotency check.
pub enum AlreadyDone {
    /// The operation completed in a previous call.
    Yes { result: String },
    /// The operation has not been seen before — proceed with execution.
    No,
}

/// Check whether an operation has already completed. Pass `stored` as
/// `Some(result)` if a lookup in your own store by `IdempotencyKey` found an
/// entry; `None` if not. A pure check — does not render anything.
pub fn already_done(stored: Option<String>) -> AlreadyDone

/// Render an already-done response for the agent. Independent of
/// `already_done()` above — call this regardless of how you detected the
/// no-op; that check function is one option, not a prerequisite.
/// Exits 0 — this is a successful no-op, not an error (exit code is the
/// caller's responsibility; this function only renders).
pub fn render_already_done(
    operation: &str,
    summary: &str,
    hints: &[Hint],
) -> String

/// Partial success: some operations completed, some failed.
pub struct PartialSuccess {
    pub completed: Vec<String>,
    pub failed:    Vec<FailedOp>,
    pub skipped:   Vec<String>,
}

pub struct FailedOp {
    pub operation: String,
    pub reason:    String,
    pub recovery:  Option<RecoveryHint>,
}

impl PartialSuccess {
    pub fn render(&self) -> String
    /// 0 when all operations completed or skipped.
    /// 1 when any operations failed.
    pub fn exit_code(&self) -> i32
}
```

`render_already_done()` renders a KV block and exits `0` (a successful no-op,
not an error), e.g. `render_already_done("create_issue", "Issue #42 already
exists with identical fields", &[Hint::new("Call get_issue with number=42 to
view it")])`:

```
operation: create_issue
status:    already_done
summary:   Issue #42 already exists with identical fields
help[1]:
  Call get_issue with number=42 to view it
```

`PartialSuccess::render()` is the most complex output in the crate. It leads
with a P4 summary line, then emits one block per outcome category, then folds
any per-op recovery hints into a trailing `help[]` block (rendered via the
same per-hint formatting `recovery::render_recovery` uses — `tool: suggestedParams:
{ key: value, ... }` — not the `"Retry {tool} with..."` phrasing or quoted
string values an earlier draft of this example showed):

```
partial_success: 2 completed, 1 failed, 1 skipped
completed[2]:
  create_issue
  add_label
failed[1]{operation,reason}:
  assign_user,"User 'ghost' not found"
skipped[1]:
  notify_team
help[1]:
  assign_user: suggestedParams: { user: alice }
```

Empty categories are omitted (no `skipped[0]` line when nothing was skipped).
The exit code is `1` because `failed` is non-empty.

---

### `resilience` — retry delay primitives

Pure computation only — no async, no network, no sleep. Callers build the
actual retry loop; michi provides the delay calculation and header parsing.

```rust
pub struct RetryConfig {
    pub max_retries:   u32,
    pub base_delay:    std::time::Duration,
    pub max_delay:     std::time::Duration,
    pub jitter_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries:   3,
            base_delay:    std::time::Duration::from_millis(500),
            max_delay:     std::time::Duration::from_secs(30),
            jitter_factor: 0.2,
        }
    }
}

/// Parse a Retry-After header value, relative to the current wall-clock time.
/// Accepts integer seconds ("120") or HTTP-date ("Wed, 21 Oct 2026 07:28:00 GMT").
/// Returns None for malformed or absent values.
pub fn parse_retry_after(header_value: &str) -> Option<std::time::Duration>

/// Like `parse_retry_after`, but takes the current time explicitly instead of
/// reading the system clock — deterministic and testable. `now` matters only
/// for the HTTP-date form; the delay-seconds form ignores it entirely.
pub fn parse_retry_after_at(header_value: &str, now: std::time::SystemTime) -> Option<std::time::Duration>

/// Compute the delay before the next retry attempt.
/// Applies exponential backoff (`base_delay * 2^attempt`) with jitter derived
/// from `jitter_seed` (caller-supplied, in [0.0, 1.0] — use a PRNG, not `rand`
/// inside michi). If `retry_after` is `Some`, the result is the larger of the
/// computed backoff and `retry_after`; either way the result is capped at
/// `max_delay`, so a server-supplied `Retry-After` can never force an
/// unbounded wait. Returns `None` when `attempt >= config.max_retries`.
pub fn next_retry_delay(
    config: &RetryConfig,
    attempt: u32,
    jitter_seed: f64,
    retry_after: Option<std::time::Duration>,
) -> Option<std::time::Duration>

/// Whether a status code is conventionally retryable.
/// Returns true for: 429, 502, 503, 504.
/// 500 is intentionally excluded — see supplement for rationale.
pub fn is_retryable_status(status: u16) -> bool
```

`RetryConfig`'s field names (`max_retries`/`base_delay`/`jitter_factor: f64`)
and `next_retry_delay`'s signature differ from earlier drafts of this section
— see `docs/superpowers/plans/2026-07-08-spec-parity.md`'s Design Decisions
table. `jitter_factor: f64` is strictly more capable than a `jitter: bool`
(supports partial jitter, not just full-jitter-or-none), and was the subject
of an adversarially-reviewed bug fix (jitter previously could exceed
`max_delay`); `next_retry_delay`'s 4-parameter signature (`config`, `attempt`,
`jitter_seed`, `retry_after`) reflects that fix plus the `retry_after`
integration below. `next_retry_delay` returns `Option` (not a bare
`Duration`) so a caller can distinguish "one more attempt, with this delay"
from "retries exhausted" — spec's "always returns at least initial_delay"
framing predates that signal.

Usage pattern for a caller implementing their own retry loop:
```rust
let config = RetryConfig::default();
for attempt in 0..config.max_retries {
    match call_api() {
        Ok(result) => return Ok(result),
        Err(e) if michi::resilience::is_retryable_status(e.status) => {
            let retry_after = e.retry_after_header
                .and_then(|h| michi::resilience::parse_retry_after(&h));
            let Some(delay) = michi::resilience::next_retry_delay(&config, attempt, jitter_seed(), retry_after) else {
                return Err(e); // retries exhausted
            };
            sleep(delay).await;
        }
        Err(e) => return Err(e),
    }
}
```

---

### `status` — content-first orientation responses (P8)

Implements **AXI Principle 8 (Content First)**: a bare tool invocation shows
live, actionable state, not a help-text wall. When an agent invokes a tool for
the first time it almost always wants current state (open issues, recent PRs,
CI status), so serving data instead of help collapses "orient + query" into a
single invocation. `StatusResponse` is what any tool returns when called with
no arguments.

Built on `kv::render_kv()`. Health signals surface degraded state alongside
nominal metrics — approaching rate limit, stale index, auth expiry — which is a
form of pre-computed aggregate (**P4**): the agent reads the summarized health
inline instead of fetching component metrics separately.

```rust
pub struct StatusItem {
    pub key:    String,
    pub value:  kv::KvValue,
    pub health: Option<Health>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Health {
    Ok,
    Degraded(String),   // "(reason)" appended inline
    Error(String),
}

pub struct StatusResponse {
    pub tool_name:   &'static str,
    pub description: &'static str,
    pub items:       Vec<StatusItem>,
    pub hints:       Vec<Hint>,
}

impl StatusResponse {
    pub fn render(&self) -> String
}
```

Example output:
```
tool:         my-search-tool
description:  Semantic code search and symbol analysis
index:        ready
files:        2847
cache:        warm (98MB / 100MB)  [DEGRADED: approaching limit]
last-updated: 4 minutes ago
help[1]:
  Run `search <query>` to search
```

---

### `recovery` — recovery hint shapes

Recovery hints are the failure-path arm of **AXI P9**: when an operation fails,
emit a concrete, parameterized "here is how to recover" template rather than a
dead end. `render_recovery()` formats them as a `help[]` block carrying
`suggestedParams`.

```rust
pub struct RecoveryHint {
    pub tool:   String,
    pub params: Vec<(String, crate::kv::KvValue)>,
    pub reason: Option<String>,
}

impl RecoveryHint {
    pub fn new(tool: impl Into<String>) -> Self
    pub fn param(mut self, key: impl Into<String>, value: KvValue) -> Self
    pub fn reason(mut self, reason: impl Into<String>) -> Self
}

/// Render recovery hints as a help[] block with suggestedParams.
pub fn render_recovery(hints: &[RecoveryHint]) -> String
```

`params` uses `kv::KvValue` rather than `serde_json::Value` — the same
zero-dependency rationale as elsewhere in this spec (see the Cargo.toml
section): `KvValue`'s `Text`/`Int`/`Float`/`Bool`/`Duration`/`Missing`
variants carry equivalent expressiveness for recovery params without pulling
in `serde_json`. This resolves Q5 below.

Text-format rendering (`render_recovery()`, the `recovery[N]:` block) always
stringifies every param value (`{ user: alice, seconds: 30 }` — no quotes,
since it's plain text, not JSON). JSON-format rendering
(`AgentResponse::render_json()`, see the `response` section) is different:
it emits each param using its *native* JSON type — `KvValue::Int`/`Float`/
`Bool` as unquoted JSON literals, `KvValue::Text` as a quoted JSON string,
`KvValue::Missing` as `null` — so a downstream JSON consumer (a TypeScript/
MCP client) receives typed values instead of every value flattened to a
string. This was a real, adversarially-found gap in an earlier
implementation (recovery params were stringified even in JSON output); it is
now fixed and covered by tests in `src/response.rs` and `src/kv/mod.rs`.

---

### `response` — AgentResponse builder

The primary integration point. Composes all modules into a single builder.
Callers use this rather than calling individual render functions directly.

```rust
/// Builder for an agent-facing response. Routes to TOON or KV based on
/// which items method is called, not on item count.
///
/// # Format routing
/// - `.items()` called    → TOON (list of uniform-schema rows)
/// - `.kv_items()` called → KV   (single item or mixed-type metadata)
///
/// Use `.items()` for 5+ uniform rows. Use `.kv_items()` for single items,
/// status data, or heterogeneous metadata.
pub struct AgentResponse {
    type_name:         String,
    items:             Vec<Vec<Value>>,
    fields:            Vec<String>,
    single_item:       Vec<kv::KvItem>,
    total_count:       Option<usize>,
    hints:             Vec<Hint>,
    recovery:          Vec<RecoveryHint>,
    truncate_cells_at: usize,
    is_error:          bool,
}

/// The serialisation format for `AgentResponse::render`. Only two variants —
/// unlike an earlier draft's three-way `Toon`/`Kv`/`Json` split, the TOON-vs-KV
/// choice is decided by which of `.items()`/`.kv_items()` was called (see
/// `target`, below), not by a value passed to `render()`. `OutputFormat` only
/// selects text vs. JSON.
pub enum OutputFormat {
    /// Plain-text TOON / kv format (whichever `.items()`/`.kv_items()`
    /// populated last). Default.
    Text,
    /// Compact JSON object — field names match the builder setters.
    Json,
}

impl AgentResponse {
    pub fn new(type_name: impl Into<String>) -> Self

    // List path (→ TOON)
    pub fn items(mut self, rows: Vec<Vec<Value>>, fields: &[&str]) -> Self
    pub fn total_count(mut self, n: usize) -> Self

    // Single-item path (→ KV)
    pub fn kv_items(mut self, items: Vec<kv::KvItem>) -> Self

    // Shared
    pub fn hint(mut self, hint: impl Into<String>) -> Self
    pub fn hints(mut self, hints: Vec<Hint>) -> Self
    pub fn recovery_hint(mut self, r: RecoveryHint) -> Self
    pub fn truncate_cells_at(mut self, limit: usize) -> Self
    /// Mark this response as an error state — reflected in `OutputFormat::Json`'s
    /// `isError` field. Beyond spec: maps directly onto MCP's `CallToolResult.isError`.
    pub fn as_error(mut self) -> Self

    pub fn render(&self, format: OutputFormat) -> String
    /// Reads the TOON slot (`items`/`fields`/`total_count`) unconditionally —
    /// not a shorthand for `render(OutputFormat::Text)`, which instead follows
    /// whichever of `.items()`/`.kv_items()` was called last. See below.
    pub fn render_toon(&self) -> String
    /// Reads the KV slot (`single_item`/`total_count`) unconditionally — the
    /// KV-path counterpart of `render_toon()`.
    pub fn render_kv(&self) -> String
    pub fn render_hints_only(&self) -> String // see supplement: three-surface seam
}
```

`total_count`/`truncate_cells_at` use `usize`, not `u64` — `u64` in an
earlier draft would have needed a lossy cast on 32-bit targets for no
benefit; `usize` matches every other length-shaped field in the crate.
`is_error: bool`/`.as_error()` and JSON's `isError` field are additions
beyond this section's earlier draft: they let `AgentResponse` map directly
onto MCP's `CallToolResult.isError` without a separate wrapper type.
`render_json(&self) -> String` is **not** a public method on `AgentResponse`
in Rust — JSON is reached via `render(OutputFormat::Json)`, matching the rest
of the crate's zero-`serde_json` design (see Cargo.toml section); it is
hand-built JSON text, not `serde_json::Value`. The NAPI wrapper (see below)
does expose a convenience `renderJson(): string` method, since TypeScript
callers benefit from not having to import an `OutputFormat` enum just to
reach the JSON path.

The `items` (TOON) and `single_item` (KV) slots are stored independently, so
the two paths never conflict at the data level. `render(format)` dispatches
on `target` — whichever of `.items()`/`.kv_items()` was called *last* — for
both `OutputFormat::Text` and `OutputFormat::Json`. The named shorthand
methods are different and stronger: `render_toon()` reads only the
`items`/`fields` slot and `render_kv()` reads only the `single_item` slot,
*regardless* of which method was called last or what `target` currently is.
Concretely: `AgentResponse::new("x").items(rows, &fields).kv_items(kv_rows).render_toon()`
still renders the TOON list from `rows`/`fields` — it does not follow
`target` (which is `Kv` here) the way `render(OutputFormat::Text)` would.
Calling both `.items()` and `.kv_items()` on one builder remains a
caller-side logic error to avoid, not a panic — but it is no longer possible
to observe *which* setter "won" by calling the two named shorthands, since
each is now slot-specific. See the supplement for the recommended discipline.

This crate has no `unsafe impl Send`/`Sync` for `AgentResponse` (an earlier
draft of this section showed one) — every field is an owned, non-interior-
mutable type (`String`, `Vec`, `Option<usize>`, `bool`, ...), so `Send`/`Sync`
already hold via Rust's automatic trait derivation. An explicit `unsafe impl`
would be both redundant and a violation of this crate's own "no `unsafe`
outside the napi boundary" rule.

---

## NAPI npm package — `michi`

The npm package is a thin napi-rs wrapper around the same crate, built with the
`napi` feature enabled. It follows the standard napi-rs dual-crate pattern:

- Feature-gated `Cargo.toml` with the `napi` feature
- `napi-rs` with `napi-derive` proc-macros — clean Rust in, C-ABI glue and
  TypeScript types out, generated at compile time
- Typed `#[napi(object)]` structs (`JsToonValue`, `JsKvItem`, ...) for the
  dynamic FFI boundary (cell values, recovery params), not `serde_json::Value`
  — same zero-`serde_json` rationale as the rest of this spec
- Platform-aware binary loading via the generated `index.js`
- TypeScript fallback export for environments without the native binary
- Cross-compiled via `cargo-zigbuild`:
  - `aarch64-apple-darwin` (`darwin-arm64`)
  - `x86_64-unknown-linux-musl` (`linux-x64-musl`)

### Why `napi` v3 / the `napi6` feature

napi-rs gates capabilities behind Node-API version features. An earlier draft
of this crate pinned `napi` v2 with the `napi4` feature — `napi4` was, at the
time, the lowest level exposing `ThreadsafeFunction` and the async support
napi-rs needs, with Node.js 12 (the first LTS with full napi4) as the floor.
The crate has since moved to `napi` v3 with the `napi6` feature — napi-rs v3
removes the old Docker-image requirement for cross-compilation, which was the
actual reason for the move (see `docs/superpowers/specs/2026-07-03-michi-design.md`
§7/Q4). michi's exports remain synchronous pure functions either way, so the
async machinery gated behind either feature level is not itself something
michi exercises — the choice is about the build tooling, not a capability
michi needs.

### The builder boundary problem and its solution

This is the one genuinely hard part of wrapping michi in napi-rs, and it is
worth spelling out. The Rust `AgentResponse` is a **consuming** builder — every
setter takes `self` by value and returns `Self`:

```rust
pub fn items(mut self, rows: Vec<Vec<Value>>, fields: &[&str]) -> Self
```

That idiom cannot cross the napi boundary directly. A `#[napi]` class instance
is owned by the JavaScript garbage collector; Rust only ever receives `&self`
or `&mut self`, never ownership, because a live JS reference still points at the
underlying struct. You therefore cannot write `pub fn items(self, ...) -> Self`
on a napi class.

The canonical fix is to store the consuming Rust builder in an `Option` slot and
`std::mem::take` it out on each mutation, applying the consuming method and
storing the result back:

```rust
#[napi(js_name = "AgentResponse")]
pub struct JsAgentResponse {
    inner: Option<michi::AgentResponse>,
}

#[napi]
impl JsAgentResponse {
    #[napi(constructor)]
    pub fn new(type_name: String) -> Self {
        Self { inner: Some(michi::AgentResponse::new(type_name)) }
    }

    /// `&mut self` setter: take the consuming builder out of the Option,
    /// apply the consuming method, put the result back. Returns `()`, not
    /// `Self` — see below for why chaining was not carried through to the
    /// shipped NAPI surface.
    #[napi(catch_unwind)]
    pub fn items(
        &mut self,
        rows: Vec<Vec<JsToonValue>>, // typed cell value, not serde_json::Value — zero-dep boundary
        fields: Vec<String>,
    ) -> napi::Result<()> {
        let b = self.inner.take()
            .ok_or_else(|| napi::Error::from_reason("response already rendered"))?;
        let field_refs: Vec<&str> = fields.iter().map(String::as_str).collect();
        let converted = convert_rows(rows);              // JsToonValue → michi::toon::Value
        self.inner = Some(b.items(converted, &field_refs));
        Ok(())
    }

    #[napi(catch_unwind)]
    pub fn render_toon(&self) -> napi::Result<String> {
        self.inner.as_ref()
            .ok_or_else(|| napi::Error::from_reason("response already consumed"))
            .map(|b| b.render_toon())
    }
}
```

The `Option` acts as a nullable ownership slot: `take()` moves the value out and
leaves `None`, the consuming method runs, and the result is reassigned.

An earlier draft of this section said the TypeScript surface presents these
as chainable `this`-returning setters. That was the plan, but it is not what
shipped, and deliberately so: every setter on the actual `JsAgentResponse`
returns `napi::Result<()>` (void on success), not `this`. Making a setter
return `this` across the napi boundary would mean returning a JS-visible
reference to the *same* wrapped object from a `&mut self` method — napi-rs's
`#[napi]` class methods don't hand back the receiver this way, so doing it
properly would require a more invasive change to the `Option`+`take()`
pattern above (e.g. holding a JS-side handle back to `self` and threading it
through every mutator's return value) for a purely cosmetic JS ergonomics
win. Given the pattern above already works correctly and is fully tested,
this was judged not worth the added complexity. Concretely, this means
TypeScript callers use sequential statements, not method chaining:

```typescript
const r = new AgentResponse("issues");
r.items(rows, ["number", "title"]);   // returns undefined, not r
r.hint("Try a broader filter");       // ditto
const out = r.renderToon();
```

not:

```typescript
// This does NOT work — items()/hint() return undefined, not `this`.
const out = new AgentResponse("issues")
  .items(rows, ["number", "title"])
  .hint("Try a broader filter")
  .renderToon();
```

(Note: async `&mut self` is unsound in napi-rs — it cannot enforce
exclusivity across the event loop — and would require an `unsafe` marker.
michi's setters are all synchronous, so this does not arise.)

### Error handling at the boundary

Any `#[napi]` function returning `napi::Result<T>` throws a JS `Error` on
`Err`. Two practices apply to michi:

- Wrap fallible conversions (row shape mismatches, oversized inputs) in
  `Result` and surface them as `napi::Error::from_reason(...)`.
- Annotate exports that call into non-trivial rendering with
  `#[napi(catch_unwind)]`. Without it, **a Rust panic crashes the entire Node
  process** — there is no isolation boundary. With it, a panic becomes a thrown
  JS `Error` (or a rejected `Promise` for async) instead of process death.

### Numeric boundary gotcha

`total_count` is `usize` in Rust (not `u64` — see the `response` section), and
the NAPI boundary narrows further to a plain `i32` (`JsToonOptions.total_count`,
`JsAgentResponse::total_count`), clamped non-negative (`n.max(0) as usize`) on
the way in. JavaScript numbers are 64-bit floats with a max safe integer of
2^53, so `i32` is comfortably within the safe range without needing the
`i64`-as-`number` mapping or `BigInt` an earlier draft of this section
anticipated; counts beyond `i32::MAX` are not a realistic concern for agent
list responses.

### Platform binary loading (`index.js`)

`@napi-rs/cli` generates `index.js` at build time. It tries a locally compiled
`.node` file first (so `napi build` during development needs no reinstall), then
falls back to the per-platform optional-dependency npm package matching the
current `process.platform` / `process.arch`. On Linux it additionally branches
on glibc vs musl via `detect-libc`. Each platform binary ships as its own npm
package listed under `optionalDependencies`, with `cpu`/`os` fields so package
managers install only the matching one. If neither a local `.node` nor an
optional dep is present, the require throws — there is no silent degradation, so
the package's TypeScript fallback export should surface a clear error.

> Known pitfall: npm sometimes omits optional platform deps from
> `package-lock.json` when the lockfile was generated on a different
> architecture (npm/cli#4828). Prefer `pnpm`, or regenerate the lockfile on the
> target arch.

### TypeScript type generation

`index.d.ts` is **never hand-written**. The `napi-derive` macro emits type
metadata and the CLI assembles the `.d.ts` at build time — function signatures,
the `AgentResponse` class, `Promise<T>` for async, `T | null` for `Option<T>`.
Where an inferred type is too loose (e.g. the dynamic cell-value arrays), the
wrapper uses `#[napi(ts_arg_type = "...")]` / `#[napi(ts_return_type = "...")]`
overrides so the published types match the hand-authored contract below.

### Cross-compilation and CI

`@napi-rs/cli`'s `--cross-compile` flag selects `cargo-zigbuild` as the linker
for non-native Linux/macOS targets (and `cargo-xwin` for Windows). The two
shipped targets build from a single Linux runner:

```bash
cargo install cargo-zigbuild
napi build --release --cross-compile --target aarch64-apple-darwin
napi build --release --cross-compile --target x86_64-unknown-linux-musl
```

The publish CI uses a `fail-fast: false` matrix that builds one `.node` per
target, uploads each as an artifact, then a publish job downloads all artifacts
and runs `napi prepublish`:

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - target: x86_64-unknown-linux-musl
        host: ubuntu-latest
        cross: true            # triggers cargo-zigbuild
      - target: aarch64-apple-darwin
        host: macos-latest     # native build (preferred for code-signing)
```

The publish job emits the per-platform packages and the main `michi` package
together. The npm package version is asserted equal to the crate version before
publish (see Versioning and release).

**Windows is a non-goal for v1.** The agent consumers michi targets (MCP
servers, CLIs invoked by coding agents) run on Linux and macOS hosts; a
`windows-x64` target would add `cargo-xwin` tooling and a third CI lane for no
current consumer. It is not blocked on anything — it can be added to the matrix
when a Windows consumer appears.

### TypeScript types (`index.d.ts`)

An earlier draft of this section sketched an ergonomic TS-first surface
(`CellValue = string | number | boolean | null`, `TruncateResult`,
`RecoveryHintParam.value: unknown`, `this`-returning `AgentResponse`
methods). The shipped surface is more literal about napi-rs's constraints —
discriminated cell values instead of a union, no structured recovery params
over the boundary, `undefined`-returning setters (see "Chainable setters,"
above) — traded for a smaller, more predictable NAPI implementation surface.
This is what `@napi-rs/cli` actually generates today:

```typescript
/** Scalar TOON/KV cell value. Discriminate via `type`. */
export interface ToonValue {
  type: "str" | "int" | "float" | "bool" | "null";
  strVal?: string;
  intVal?: number;
  floatVal?: number;
  boolVal?: boolean;
}

export interface RenderToonOptions {
  typeName: string;
  fields: string[];
  /** Rows, each an array of values parallel to `fields`. */
  rows: ToonValue[][];
  totalCount?: number;
  hints?: string[];
}

/** Render a list of items as TOON. Throws if rows/fields/hints, or any row,
 * exceed this module's per-call size limits (protects Node's single-threaded
 * event loop from unbounded synchronous allocation on a crafted call). */
export declare function renderToon(opts: RenderToonOptions): string;

/** Render a definitive empty-state TOON response: `typeName[0]{}:\ntotalCount: 0\n`.
 * No `hints` parameter over this boundary — use `appendHints()` to add one. */
export declare function emptyState(typeName: string): string;

/** Build a standalone help[] block. Returns empty string when hints is empty. */
export declare function renderHints(hints: string[]): string;

/** Append a help[] block to an existing body string. */
export declare function appendHints(body: string, hints: string[]): string;

/** Truncate content to at most maxChars Unicode scalar values, with `hint`'s
 * truncation signal appended inline. Returns the final string directly —
 * unlike the Rust `truncate()`, which returns a richer `Truncated` struct,
 * only the inline form crosses the NAPI boundary. */
export declare function truncate(content: string, maxChars: number, hint: string): string;

export interface RecoveryHintInput {
  tool: string;
  /** No structured params over this boundary — use `AgentResponse.recoveryHint()`
   * for the common "what to call next" case, or the Rust API directly (which
   * accepts typed `KvValue` params) for anything richer. */
  reason?: string;
}

/** Render recovery hints as a recovery[N]: block. */
export declare function renderRecovery(hints: RecoveryHintInput[]): string;

/** High-level builder — mirrors the Rust `AgentResponse` API. Every mutator
 * returns `undefined`, not `this` — see "Chainable setters," above; callers
 * use sequential statements, not method chaining. */
export declare class AgentResponse {
  constructor(typeName: string);
  items(rows: ToonValue[][], fields: string[]): void;
  totalCount(n: number): void;
  kvItems(items: { key: string; value: ToonValue }[]): void;
  hint(hint: string): void;
  recoveryHint(tool: string, reason?: string): void;
  /** Marks this response as an error state, reflected in `renderJson()`'s `isError` field. */
  asError(): void;
  /** Reads the TOON slot unconditionally — see the `response` section. */
  renderToon(): string;
  /** Reads the KV slot unconditionally — see the `response` section. */
  renderKv(): string;
  /** Returns a JSON *string* — `{"body":...,"hints":[...],"recovery":[...],"isError":bool}`
   * — not a parsed value, matching the Rust side's zero-`serde_json` design. */
  renderJson(): string;
  renderHintsOnly(): string;
}
```

---

## What michi provides vs what stays in your application

### Formalising conventions that are typically implicit

| Concept | Where it usually lives | Provided as |
|---|---|---|
| Contextual hints after output | Implicit strings per tool | `hints::Hint`, `render_hints()`, `append_hints()` |
| Total count on list responses | Number passed ad-hoc | `ToonOptions::total_count`, `totalCount:` line |
| Recovery hints on failure | Ad-hoc per server/tool | `recovery::RecoveryHint`, `render_recovery()` |
| Retry delay calculation | Inside caller's retry loop | `resilience::next_retry_delay()` |
| Retry-After header parsing | Ad-hoc string parsing | `resilience::parse_retry_after()` |
| Retryable status classification | Hardcoded booleans per caller | `resilience::is_retryable_status()` |
| Error classification shape | Ad-hoc per tool | `error::Error`, `error::DomainError`, `error::ErrorCode` |
| Empty state handling | Ad-hoc per tool (often silent) | `empty::empty_state()` |
| Truncation | Ad-hoc per tool, no standard signal | `truncate::truncate()`, `truncate_inline()` |

### Genuinely new — does not exist as a shared primitive today

| What | Why |
|---|---|
| TOON renderer | New format — token-efficient list encoding for agent context windows |
| `kv::render_kv()` | Single-item Markdown-KV with aligned columns; no standard Rust equivalent |
| `status::StatusResponse` | Typed P8 content-first orientation response — novel primitive |
| `idempotency::already_done()` | Typed already-done signal with canonical output format |
| `idempotency::PartialSuccess` | General partial-success reporting with per-op recovery hints |
| `idempotency::IdempotencyKey` | Canonical key construction with stable hashing |
| `AgentResponse` builder | Unified construction API across all consumers |
| NAPI wrapper | Cross-language bridge — TypeScript authors never write Rust |
| Formal TOON grammar | Canonical spec enabling interoperability and testability |
| `render_hints_only()` | Append hints to an existing body without re-rendering |

### Stays in your application code

| What | Why |
|---|---|
| Display Markdown formatters | Display-surface formatting; `audience: ["user"]` |
| MCP `content[]` assembly | Protocol knowledge |
| `outputSchema` wiring | Tied to your schema validation library |
| Tool annotations | MCP SDK concern |
| MCP server bootstrapping | Protocol-specific |
| HTTP client + auth | Operational concerns with too many deployment shapes |
| LRU cache | General utility, not AXI-specific |
| Full async retry loop | Requires async runtime |
| Structured logging | `tracing` or similar, caller's choice |
| Schema validation | Tied to your type system |

---

## Feature flags

```toml
[features]
default = []
napi = ["dep:napi", "dep:napi-derive"]
cli  = []  # reserved: terminal-width-aware rendering (colours, wrap)
```

No async runtime dependency. No tokio, no async-std. All public functions are
sync. The `cli` feature is reserved for terminal-aware rendering (line
wrapping, colour codes for the `[DEGRADED: ...]` health signals in
`status::StatusResponse`) — out of scope for v1. michi v1 targets agent
consumers only. When the `cli` feature is fleshed out it will pull in a
terminal crate (`crossterm` for width detection and styling, or `colored` for
the minimal case) gated entirely behind the flag so the default build stays
dependency-light; see Open question Q3 for the v2 scope sketch.

---

## Versioning and release

- Published to [crates.io](https://crates.io) as `michi`
- npm package published to [npmjs.com](https://npmjs.com) as `michi`
- NAPI binary cross-compiled via `cargo-zigbuild`:
  - `darwin-arm64` (`aarch64-apple-darwin`)
  - `linux-x64-musl` (`x86_64-unknown-linux-musl`)

**SemVer contract:**

| Bump | Meaning |
|---|---|
| **Patch** | Bug fixes; TOON output identical or more correct |
| **Minor** | New API surface (new module, new method on `AgentResponse`) |
| **Major** | TOON format change; any existing rendered output would differ |

Format changes are major versions. Consumers should treat the rendered string
as a contract — the snapshot tests exist to make accidental format drift a
failing build rather than a silent breaking change.

**MSRV policy:** `rust-version = "1.93"` is the declared minimum. An MSRV bump
happens only when a needed language or `std` feature requires it, is recorded in
the CHANGELOG, and is treated as a **minor** version bump (never a patch).

**Version sync:** the npm package version tracks the crate version exactly. CI
asserts the two are equal before any publish, so a `michi` crate at `0.3.1` and
the `michi` npm package at `0.3.1` always describe the same source. The publish
job builds the per-platform `.node` artifacts, runs `napi prepublish` to emit
the platform packages as `optionalDependencies`, then publishes the main
package; when no native binary matches a consumer's platform, the package's
TypeScript fallback export is what loads.

---

## Performance contract

| Operation | Target | Basis |
|---|---|---|
| `render_toon()` — 100 items, 4 fields | < 500µs | Simple string allocation, no I/O |
| `render_toon()` — 1000 items, 4 fields | < 3ms | Linear in N×fields |
| `render_kv()` — 10 items | < 20µs | Column-width scan + string join |
| `render_hints()` — 5 hints | < 10µs | Trivial string join |
| `truncate_inline()` | < 5µs | Single pass, char boundary safe |
| `parse_retry_after()` | < 2µs | String parse only |
| `next_retry_delay()` | < 1µs | Pure arithmetic |
| NAPI boundary overhead | < 5µs | Established from napi-rs benchmarks |

**Allocation strategy:** `render_toon()` uses `String::with_capacity` estimated
from `items.len() × avg_row_estimate` before any writes (see the supplement for
the exact heuristic). All functions are allocation-bounded — no references held
across call boundaries. The `escape.rs` module avoids heap allocation for the
common case (no special chars in a cell).

**Thread safety:** All public functions are stateless pure functions.
`AgentResponse` is `Send + Sync` (all fields are owned, no interior
mutability). The NAPI wrapper uses napi-rs's safe threading model.

Benchmarks via `divan` (not `criterion` — see the Cargo.toml section). Run in
CI on PRs touching `src/`.

---

## Testing strategy

### Unit tests (in-crate, `#[cfg(test)]`)
- TOON grammar: one test per grammar production
- KV: alignment, unicode keys, missing values, all `KvValue` variants
- Escaping: commas, quotes, null, empty string, unicode
- Truncation: exact boundary, under, over, unicode char boundary safety
- Hints: empty, single, multiple, long strings
- Empty state: with and without hints
- Recovery: single, multiple, with/without params and reason
- Error: each `ErrorCode` variant renders + exit codes + `Display` impl
- Idempotency: `already_done` format, `PartialSuccess` with mixed results
- Resilience: `parse_retry_after` — integer seconds, HTTP-date, malformed;
  `next_retry_delay` — backoff progression, jitter bounds, Retry-After respect;
  `is_retryable_status` — all covered codes + non-covered codes
- Status: all health states, degraded rendering, error rendering
- `AgentResponse` builder: all method combinations, correct format dispatch

### Property tests (`proptest`, `tests/`)
- `render_toon()` output is valid per grammar for arbitrary string inputs
- `truncate_inline()` never returns string longer than `limit + signal_len`
- TOON round-trip: render → parse (test-only parser in `tests/`) → compare values
- `parse_retry_after()` never panics on arbitrary strings
- `next_retry_delay()` always returns value within `[initial_delay, max_delay]`

### Snapshot tests (`insta`, `tests/snapshot_tests.rs`)
- Canonical TOON examples from this spec — exact byte-for-byte match
- KV column alignment across different key lengths
- Status response with mixed health signals
- Format stability — prevents accidental format drift across versions

### NAPI integration tests (Node's built-in `node:test`, `packages/michi-node/__test__/`)

An earlier draft named Jest here; the shipped test runner is Node's built-in
`node:test` (`pnpm test` → `node --test`), already wired into CI — switching
test runners now would be unrelated churn for no behavioral benefit.
- Each NAPI export with representative inputs
- Error cases: mismatched row lengths, oversized inputs, null values
- `AgentResponse` builder via NAPI, all paths (including the `Option`/`take`
  "already rendered" guard)
- Platform binary loading on CI matrix (darwin-arm64, linux-x64-musl)

### Consumer integration tests
- MCP consumer: `audience: ["assistant"]` block is valid TOON
- CLI consumer: `--format toon` output is valid TOON

---

## Open questions

**Q1 — TOON vs Markdown-KV for agent-facing list responses**
AXI benchmarks the efficiency gains at the CLI/MCP level, but not the
downstream LLM retrieval accuracy of TOON vs Markdown-KV for list data.
Before treating TOON as universally superior for the `audience: ["assistant"]`
surface, run a retrieval accuracy experiment: same list data, TOON vs
Markdown-KV, retrieval task, across a few model sizes. If Markdown-KV wins
on retrieval accuracy despite higher token cost, the right call for MCP
consumers may be to use Markdown-KV even for lists. The crate ships either
way — the question is which format callers should default to.

**Q2 — External Rust consumer strategy**
For external Rust binaries to depend on `michi`, options:
(a) crates.io publish — cleanest for public consumers once API is stable
(b) git dep with tag: `michi = { git = "https://github.com/orin-axi/michi", tag = "v0.x.y" }`
    — good for early adopters before crates.io publish
(c) private cargo registry — overhead not worth it

Recommend (b) during development, (a) at first stable release. Gate
crates.io publish behind at least one real consumer integration test.

**Q3 — `cli` feature scope**
Reserved for terminal-aware rendering. If `--format toon` in a human
terminal context should wrap long rows or colourize the header, the `cli`
feature is where that lives. The likely v2 surface is a `render_terminal()`
variant that consults terminal width (via `crossterm`) and applies ANSI colour
to health signals and the type header. Out of scope for v1 — michi v1 targets
agent consumers only, not human-readable terminal output.

**Q4 — `AgentResponse` builder vs standalone functions (resolved)**
Implemented per this question's own recommendation, plus one addition:
`AgentResponse`/`JsAgentResponse` is exported, along with
`renderHints()`/`appendHints()`/`renderRecovery()`/`renderToon()`/
`emptyState()`/`truncate()` — the low-level functions are exported
*additively*, not instead of the builder, since removing an already-shipped
export is a breaking change with no upside. Rust-only consumers can still
reach every primitive directly; TypeScript consumers get both the builder and
the standalone functions.

This resolution also settled the "chainable builder" question the original
spec draft assumed an answer to: `JsAgentResponse`'s setters return
`undefined`, not `this` (see the NAPI section's "Chainable setters" note).
Making them `this`-returning across the napi boundary was judged real,
risky rework for a cosmetic JS ergonomics win, with the JS test suite already
proving sequential-statement usage works fine, including mutating after a
render call.

**Q5 — Recovery hint format (resolved)**
Resolved by using `kv::KvValue` (an existing, already-tested internal enum)
instead of `serde_json::Value` for `RecoveryHint.params` — see the `recovery`
section. This fixes the *Rust-side* type-loss concern Q5 raised without
adding a dependency `serde_json::Value` would have required.

A second, related concern surfaced during this reconciliation and is now
also resolved: independent of which Rust-side type `params` uses,
`AgentResponse::render_json()`'s JSON *output* was, for a time, stringifying
every param value regardless of its `KvValue` variant (`{"seconds":"30"}`
instead of `{"seconds":30}`), which would have defeated the point of using a
typed enum in the first place. `render_json()` now serializes `KvValue::Int`/
`Float`/`Bool` as native JSON literals, `Text` as a JSON string, and
`Missing` as `null` — so both the Rust-side representation and the
JSON-output representation carry real type information, not just the
former.

---

## Supplement — implementation notes and design rationale

### `AgentResponse` format routing

The builder routes to TOON or KV based on which items method the caller uses,
not on item count. The caller is responsible for choosing the right format:

```rust
// List of uniform-schema items → TOON
AgentResponse::new("issues")
    .items(rows, &["number", "title", "state"])   // → render_toon()

// Single item or mixed-type metadata → KV
AgentResponse::new("issue")
    .kv_items(vec![
        KvItem { key: "number".into(), value: KvValue::Int(42) },
        KvItem { key: "title".into(),  value: KvValue::Text("Fix login".into()) },
    ])                                             // → render_kv()
```

**Guidance:** use `.items()` when all rows share the same schema and there are
5 or more of them — TOON's token savings compound with N. Use `.kv_items()` for
single items, mixed-type status data, or any case where the field names differ
per row. A single-item `.items()` call works but produces a TOON header with one
row, which is less readable and offers no token advantage over KV.

The `items` and `single_item` slots are independent, so the two paths cannot
corrupt each other's data. If both are populated, the render method (or the
`OutputFormat` passed to `render()`) decides which slot is read; the other is
ignored. Populating both on one builder is a caller-side logic error — the
builder neither panics nor silently merges. Treat one `AgentResponse` as one
output shape.

---

### `IdempotencyKey` — stable input serialisation

`IdempotencyKey::from_hash(operation, data)` hashes the raw bytes of `data`.
For the key to be canonical across calls with the same logical input, `data`
must be produced deterministically:

- **Maps and structs:** sort keys alphabetically before serialising to JSON.
  `serde_json::to_vec` on a `HashMap` produces non-deterministic key order.
  Use `BTreeMap` or a sorted intermediate representation.
- **Floats:** avoid floating-point values in idempotency keys — representation
  varies by architecture. Round to a fixed decimal or convert to integers.
- **Timestamps:** exclude request-time fields (created_at, request_id) unless
  the intent is a per-request key rather than a per-operation key.

`from_hash` takes raw `&[u8]` — michi has no opinion on how you produce those
bytes, and (per the Cargo.toml section) does not itself depend on
`serde_json`. The example below uses it only because it is a common choice
for callers already producing JSON elsewhere; any deterministic
serialization (a hand-rolled sorted-field formatter, `bincode`, etc.) works
equally well, as long as it is sorted-key-deterministic per the bullets
above.

```rust
// Correct: BTreeMap serialises with sorted keys
let mut params: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
params.insert("project", json!("PROJ"));
params.insert("type",    json!("Task"));
let key = IdempotencyKey::from_hash("create_item", &serde_json::to_vec(&params)?);

// Incorrect: HashMap key order is non-deterministic
let params: HashMap<&str, _> = /* ... */;
let key = IdempotencyKey::from_hash("create_item", &serde_json::to_vec(&params)?);
// ^ same logical input may produce different keys on different calls
```

---

### `is_retryable_status()` — why 500 is excluded

The default retryable set is `{ 429, 502, 503, 504 }`. HTTP 500 (Internal
Server Error) is intentionally absent.

429 and 50x gateway errors (502, 503, 504) represent transient conditions —
rate limits and upstream unavailability — where the same request is likely to
succeed after a delay. 500 represents a server-side bug. Retrying a 500 without
changing anything about the request will produce the same 500. Worse, retrying
write operations that returned 500 may produce duplicate side effects if the
server processed the request before erroring.

Callers that genuinely need to retry 500s (some APIs use it for transient
conditions) can implement their own `is_retryable` predicate and pass it to
their retry loop independently of `is_retryable_status()`.

```rust
// Default: 429, 502, 503, 504 only
if is_retryable_status(status) { ... }

// Custom: caller adds 500 for a specific API known to use it transiently
let retryable = is_retryable_status(status) || status == 500;
```

---

### `render_hints_only()` — the three-surface seam

This method exists to support MCP frameworks that implement a three-surface
response pattern: one content block for agent consumption
(`audience: ["assistant"]`), one for display (`audience: ["user"]`), and a
`structuredContent` payload for client tooling. In this pattern, the display
body is rendered via the framework's own Markdown or rich-text layer, while the
agent-facing block combines TOON output with a `help[]` trailer.

Without `render_hints_only()`, framework code that has already rendered a
display body would need to reconstruct an `AgentResponse` just to get a
formatted `help[]` block to append to the agent surface.

```typescript
// Example: MCP framework TypeScript (simplified)
function assembleToolResult(opts: RespondOpts): ToolResult {
  const userBody      = renderDisplayMarkdown(opts.body);    // display surface → audience:["user"]
  const hintsBlock    = michi.renderHintsOnly(opts.hints);   // michi → append to assistant body
  const assistantBody = opts.assistant                       // caller-provided TOON
      ?? (userBody + "\n" + hintsBlock);                     // fallback: display body + hints

  return {
    content: [
      { type: "text", text: assistantBody,
        annotations: { audience: ["assistant"], priority: 1.0 } },
      { type: "text", text: userBody,
        annotations: { audience: ["user"],      priority: 0.5 } },
    ],
    structuredContent: opts.structured ?? null,
  };
}
```

`render_hints_only()` is the narrow seam between michi and the calling
framework's rendering layer — it allows the `help[]` format to be owned by
Rust without requiring the framework to understand TOON.

---

### `parse_retry_after()` — format details

Accepts either of the two forms defined in RFC 7231 §7.1.3:

| Form | Example | Notes |
|---|---|---|
| Integer seconds | `"120"` | Seconds to wait from response time |
| HTTP-date | `"Wed, 21 Oct 2026 07:28:00 GMT"` | Absolute datetime, UTC only |

Returns `None` for any malformed value. Callers should treat `None` as "use
backoff only" and proceed with `next_retry_delay()` passing `retry_after: None`.

Does not validate that an HTTP-date is in the future. If a server returns a
past date (clock skew or bug), `parse_retry_after()` will return a zero or very
small duration. `next_retry_delay()` internally clamps: the returned duration
is always at least `config.initial_delay` and at most `config.max_delay`,
regardless of what `parse_retry_after()` returns.

```rust
let retry_after = parse_retry_after(header_value);
let delay = next_retry_delay(attempt, &config, retry_after);
// next_retry_delay() already clamps to initial_delay minimum — no extra handling needed
```

---

### `render_toon()` — capacity estimate (`avg_row_estimate`)

The pre-allocation in `render_toon()` is a heuristic, not a hard bound. The
estimate is:

```
capacity = header_len + items.len() × (fields.len() × AVG_CELL_BYTES)
```

where `AVG_CELL_BYTES` is an internal constant of `16` (a typical short cell:
an id, a short title fragment, a state word). The header term covers the type
name, the bracketed count, the brace-wrapped field list, and the trailing
`totalCount:`/`help[]` lines. The estimate intentionally errs slightly high so
the common case never reallocates; a wildly under-estimated row (very long
untruncated cells) may trigger one reallocation, which is acceptable because
`max_cell_len` truncation caps the realistic upper bound. `AVG_CELL_BYTES` is
not configurable — it is a tuning constant, not part of the public contract.

---

### `outputSchema` is in the LLM context

A precision point that matters for token budget planning in MCP servers that
use progressive disclosure or deferred tool loading:

- **`outputSchema`** on a tool definition is part of the `tools/list` response.
  It IS injected into the LLM context window at session start alongside
  `inputSchema`. It adds schema tokens — roughly +15% on the schema component
  (an approximate, workload-dependent figure, not a measured constant). It is a
  one-time cost that amortises across the session as a prepaid reasoning
  contract.
- **`structuredContent`** in a tool result is client-consumed only. It does
  NOT enter the LLM context window.

This matters for deferred tool loading: adding `outputSchema` to tools that are
not yet loaded would add schema tokens before those tools are called, defeating
the purpose of deferral. `outputSchema` should only be defined on always-loaded
tools, or it should be excluded from `tools/list` for deferred tools and
supplied only when the tool is actually invoked.

If your MCP framework auto-registers `outputSchema` from tool definitions,
ensure it respects the deferred/always-loaded boundary when constructing
`tools/list` responses.

---

### Build-time P4 — pre-computed aggregates at CI time

AXI Principle 4 (pre-computed aggregates) is typically discussed as a runtime
convention: always return `totalCount`, pre-filter before responding, emit
summaries instead of raw data. But the same principle applies at build time,
and that application is more powerful. Trajectory analysis identified
pre-computed aggregates as one of the highest-ROI AXI principles, and pushing
the computation to CI is where the largest wins are.

**The pattern:** any deterministic computation that a tool needs repeatedly is
a candidate for pre-computation in CI rather than per-request execution at
runtime.

**Canonical example: a build-time component index**

Consider a design system component library. At build time, a bundler plugin
can scan every component's definitions, resolve references, and write a static
lookup index shipped with the MCP server package. When an agent calls
`searchComponents()`, the handler reads from that static structure — no
scanning, no resolution, no index construction per request.

The agent pays zero turn cost for the computation. The CI pipeline pays it
once. The result is faster responses, lower token cost per call, and no
variance in response time.

**When to apply it:**

| Computation | Runtime | Build time |
|---|---|---|
| Filtering by user input | ✓ | — |
| Counting available items | Runtime (`totalCount` field) | Build time (pre-count, embed in manifest) |
| Resolving references between definitions | — | ✓ (bundler plugin) |
| Indexing for search / BM25 | — | ✓ (write index to disk at build) |
| Building dependency graphs | — | ✓ (static graph serialised to JSON) |
| Generating tool output schemas | — | ✓ (schema library `toJsonSchema()` at registration) |

Build-time P4 is a design decision, not a code pattern — it shows up during
architecture planning, not implementation. Ask: "does this computation change
per-request, or is it stable across many requests?" Stable computation belongs
at build time.
