# Rust API

## Crate root (`src/lib.rs`)

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

This is the pure-primitives (Plan 1) surface. The crate additionally has `pipeline` and
`telemetry` modules, and (behind the `napi` feature) a `napi` module. `Cargo.toml`'s only
optional features are `napi` and `serde` — the async execution layer ("Plan 2" in `CLAUDE.md`:
the pipeline executor, `fuzzy`, `cache`, `cli`) doesn't exist as code or as Cargo features on
this crate at all. It lands as genuinely separate crates depending on `michi`, built when each
piece is actually implemented — see [`ARCHITECTURE.md`](../../ARCHITECTURE.md) and
[06-decisions.md](06-decisions.md) for the reasoning. (The `sink` module mentioned in earlier
drafts of this section was removed — it held no real code, only a Plan 2 placeholder comment.)

---

## `toon` — list rendering for N uniform-schema items

TOON is the agent-facing list format (**AXI P1**) — see
[02-toon-format.md](02-toon-format.md) for the grammar. Prefer it over key-value for lists of 5+
items, where token savings compound; for single items, use `kv::render_kv()`.

```rust
pub struct ToonOptions {
    /// Snake_case type name, e.g. "issue", "component".
    pub type_name: String,
    /// Ordered field names for the header.
    pub fields: Vec<String>,
    /// Rows, each a Vec of values parallel to `fields`.
    pub rows: Vec<Vec<Value>>,
    /// Total items available. May exceed rows rendered.
    /// Emitted as "totalCount: N" when Some — the canonical AXI P4
    /// (pre-computed aggregate): tells the agent how many items exist
    /// without a follow-up call.
    pub total_count: Option<usize>,
    /// Agent-facing usage hints. Emitted as a `help[N]:` block when non-empty.
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
/// In debug builds, panics if any row's length doesn't match `fields.len()`
/// — a development-time correctness signal. Release builds render whatever
/// values a mismatched row has; the function has never indexed by field
/// position, so there's nothing to skip.
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

Every scalar variant has a `From` conversion, so callers build rows with `.into()` uniformly.
`render_toon()` pre-allocates output capacity via `String::with_capacity` (see
[06-decisions.md](06-decisions.md) for the exact heuristic) — this is the hot path, no
intermediate allocations. `escape.rs` handles comma/quote escaping inline, with no heap
allocation for the common case of a cell with no special characters.

---

## `kv` — single-item key-value rendering

Markdown key-value format for single items and small mixed-type metadata sets. Column width is
determined by the longest key.

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

## `hints` — contextual disclosure blocks

Implements **AXI Principle 9 (Contextual Disclosure)**: append a `help[]` section after every
output, suggesting logical next steps as concrete, parameterized command templates. Carry
forward fixed disambiguating flags from the current context, but leave runtime values as named
placeholders (`<id>`) rather than guessing them. This kills the discovery round trip — the agent
sees its next move in the output it just got, instead of guessing subcommands or issuing a
separate `--help` call.

```rust
/// A single next-step hint appended after tool output.
/// Should be concrete: include tool name and parameter template.
///
/// Good:  "Call get_issue with number=<number> for full detail"
/// Avoid: "Use another tool for more information"
pub struct Hint(pub String);

impl Hint {
    pub fn new(s: impl Into<String>) -> Self
    pub fn as_str(&self) -> &str
}

impl From<&str>   for Hint { ... }
impl From<String> for Hint { ... }

/// Render a standalone help[] block.
/// Returns an empty string when hints is empty — callers can
/// safely append the result without a guard.
pub fn render_hints(hints: &[Hint]) -> String

/// Append a help[] block to an existing string in place, without allocating
/// an intermediate buffer. No-op when `hints` is empty.
pub fn append_hints(out: &mut String, hints: &[Hint])
```

---

## `truncate` — size-bounded output with escape hatches

Implements **AXI Principle 3 (Content Truncation)**: large text fields get capped to a
configurable character limit with a size hint that explains the truncation and names an escape
hatch (`use full=true`). One verbose field can otherwise burn thousands of tokens, exhausting the
budget before the agent has read the rest of a list.

```rust
pub struct Truncated {
    pub content:       String,
    pub original_len:  usize,
    pub was_truncated: bool,
    /// "(N chars truncated — use {hint})" — None when not truncated
    pub signal:        Option<String>,
}

/// Truncate content to at most `max_chars` Unicode scalar values, appending
/// an agent-readable suffix (using `hint` as the escape-hatch flag name) when
/// truncation occurs. Respects char boundaries — never splits a Unicode
/// scalar. Returns the original string unchanged when it already fits.
pub fn truncate(content: &str, max_chars: usize, hint: &str) -> Truncated

/// Truncate content for inline use (e.g. inside a TOON field).
/// Returns the final string directly.
pub fn truncate_inline(content: &str, max_chars: usize, hint: &str) -> String
```

`hint` names the escape-hatch flag embedded in the truncation suffix — michi has no opinion on
what your "give me the untruncated version" flag is called, so it's a parameter, not a hardcoded
`"full=true"`.

---

## `empty` — definitive zero states

Implements **AXI Principle 5 (Definitive Empty States)**: when a query returns nothing, emit an
explicit zero-result message instead of silent empty output. Agents can't reliably tell "no
results" from "command failed silently" — an explicit `[0]` count plus `totalCount: 0` removes
the ambiguity that otherwise drives retry loops.

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

## `error` — structured errors with exit codes

Implements **AXI Principle 6 (Structured Errors and Exit Codes)**: errors go to **stdout** (not
stderr), carry a clean exit code (`0` success, `1` error), carry recovery hints, and never block
on an interactive prompt. Agents can't answer `Are you sure? [y/n]`, may not capture stderr at
all, and can't reliably parse freeform error text. `DomainError` encodes that contract as a type.

The HTTP status → `ErrorCode` mapping isn't in michi — callers produce an `ErrorCode` after
interpreting whatever failed. That keeps this module free of HTTP knowledge.

`DomainError` is the pure data/render type below. `Error` is a `thiserror`-derived enum wrapping
it — a `Domain(DomainError)` variant alongside always-compiled `InvalidInput(String)`/
`NotFound(String)` variants, with room for execution-layer variants like `Http`/`Timeout`/
`StepFailed` that need `#[source]`-chaining once the `pipeline` crate (Plan 2) lands — see
[06-decisions.md](06-decisions.md). `Error::render()`/`class()`/`exit_code()` delegate to
`DomainError`'s when the variant is `Domain`.

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
/// `Timeout`, etc.) return when the `pipeline` crate (Plan 2) lands — see
/// `docs/spec/06-decisions.md`'s crate-boundary entry.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    InvalidInput(String),
    NotFound(String),
    Domain(DomainError),
    // ...execution-layer variants behind `pipeline` — out of scope here.
}
```

`ErrorCode` is `Copy + Eq`, renders as snake_case. `DomainError::render()` produces a KV-shaped
block on stdout — for
`DomainError::new(ErrorCode::NotFound, "Issue #9999 does not exist in this repository").hint("Call list_issues to see available numbers")`:

```
error: not_found
message: Issue #9999 does not exist in this repository
exit_code: 1
help[1]:
  Call list_issues to see available numbers
```

---

## `idempotency` — already-done signals and partial success

Types and rendering for idempotency checks, already-done detection, and partial-success
reporting — the mutation-safety half of **AXI P6**. Write operations must be idempotent so an
agent can retry a transient failure without duplicating records. The caller owns the persistence
layer that tracks what's been done; michi owns the rendering contract.

Checking and rendering are two independent functions, not one — michi has no persistence layer
and can't itself decide whether an operation already happened. Only your store knows that.

```rust
/// A canonical idempotency key.
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    /// Construct from any string-like value — already-combined key string.
    /// Callers wanting an operation-name-plus-input key build it themselves
    /// (e.g. `format!("{operation}:{stable_input}")`) before calling `new`,
    /// or use `from_hash` below.
    pub fn new(s: impl Into<String>) -> Self
    /// Construct from an operation name and raw input bytes, hashed with
    /// FNV-1a for a stable, deterministic, low-collision key. Not
    /// cryptographic — idempotency keys need stability, not security.
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
/// entry; `None` if not. A pure check — renders nothing.
pub fn already_done(stored: Option<String>) -> AlreadyDone

/// Render an already-done response for the agent. Independent of
/// `already_done()` above — call this regardless of how you detected the
/// no-op. Exits 0 — a successful no-op, not an error (the exit code itself
/// is the caller's responsibility; this function only renders).
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

`render_already_done()` renders a KV block and exits `0` — e.g.
`render_already_done("create_issue", "Issue #42 already exists with identical fields", &[Hint::new("Call get_issue with number=42 to view it")])`:

```
operation: create_issue
status:    already_done
summary:   Issue #42 already exists with identical fields
help[1]:
  Call get_issue with number=42 to view it
```

`PartialSuccess::render()` is the most involved output in the crate — a P4 summary line, one
block per outcome category, then per-op recovery hints folded into a trailing `help[]` block
(rendered with the same `tool: suggestedParams: { key: value, ... }` shape `recovery::render_recovery`
uses):

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

Empty categories are omitted — no `skipped[0]` line when nothing was skipped. Exit code is `1`
because `failed` is non-empty.

---

## `resilience` — retry delay primitives

Pure computation only — no async, no network, no sleep. Callers build the actual retry loop;
michi provides the delay calculation and header parsing.

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

/// Compute the delay before the next retry attempt. Applies exponential
/// backoff (`base_delay * 2^attempt`) with jitter derived from `jitter_seed`
/// (caller-supplied, in [0.0, 1.0] — bring your own PRNG, michi doesn't
/// depend on `rand`). If `retry_after` is `Some`, the result is the larger of
/// the computed backoff and `retry_after`; either way it's capped at
/// `max_delay`, so a server-supplied `Retry-After` can never force an
/// unbounded wait. Returns `None` once `attempt >= config.max_retries`.
pub fn next_retry_delay(
    config: &RetryConfig,
    attempt: u32,
    jitter_seed: f64,
    retry_after: Option<std::time::Duration>,
) -> Option<std::time::Duration>

/// Whether a status code is conventionally retryable.
/// Returns true for: 429, 502, 503, 504.
/// 500 is intentionally excluded — see 06-decisions.md.
pub fn is_retryable_status(status: u16) -> bool
```

`next_retry_delay` returns `Option`, not a bare `Duration`, so a caller can tell "one more
attempt, with this delay" apart from "retries exhausted."

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

## `status` — content-first orientation responses (P8)

Implements **AXI Principle 8 (Content First)**: a bare tool invocation shows live, actionable
state, not a wall of help text. An agent invoking a tool for the first time almost always wants
current state — open issues, recent PRs, CI status — so serving data instead of help collapses
"orient + query" into one call. `StatusResponse` is what any tool returns when called with no
arguments.

Built on `kv::render_kv()`. Health signals surface degraded state alongside nominal metrics —
approaching a rate limit, a stale index, an expiring auth token — which is itself a pre-computed
aggregate (**P4**): the agent reads summarized health inline instead of fetching component
metrics separately.

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
    pub tool_name:   String,
    pub description: String,
    pub items:       Vec<StatusItem>,
    pub hints:       Vec<Hint>,
}

impl StatusResponse {
    pub fn new(tool_name: impl Into<String>, description: impl Into<String>, items: Vec<StatusItem>) -> Self
    /// Attach contextual hints.
    pub fn with_hints(mut self, hints: Vec<Hint>) -> Self
    pub fn render(&self) -> String
}
```

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

## `recovery` — recovery hint shapes

Recovery hints are the failure-path arm of **AXI P9**: when an operation fails, emit a concrete,
parameterized "here's how to recover" template instead of a dead end. `render_recovery()` formats
them as a dedicated `recovery[N]:` block carrying `suggestedParams` — deliberately not folded
into the generic `help[]` block. `AgentResponse.recovery: Vec<RecoveryHint>` can carry several
structured recovery hints per response; mixing those into the same block a plain next-step hint
uses would lose a downstream parser's ability to tell "next-step suggestion" from "how to recover
from this specific failure" apart.

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

/// Render recovery hints as a recovery[N]: block with suggestedParams.
///
/// recovery[2]:
///   assign_user: suggestedParams: { user: alice } — user 'ghost' not found
///   list_issues
pub fn render_recovery(hints: &[RecoveryHint]) -> String
```

`params` uses `kv::KvValue`, not `serde_json::Value` — same zero-dependency reasoning as the rest
of the crate. `KvValue`'s `Text`/`Int`/`Float`/`Bool`/`Duration`/`Missing` variants carry
equivalent expressiveness without pulling in `serde_json`.

Text-format rendering (`render_recovery()`) stringifies every param value (`{ user: alice,
seconds: 30 }` — no quotes, plain text). JSON-format rendering
(`AgentResponse::render_json()`) is different: it emits each param using its *native* JSON
type — `Int`/`Float`/`Bool` as unquoted literals, `Text` as a quoted string, `Missing` as
`null` — so a downstream JSON consumer gets typed values instead of everything flattened to a
string.

---

## `response` — AgentResponse builder

The primary integration point. Composes every module into a single builder — use this instead of
calling individual render functions directly.

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
    /// Internal only — which of `.items()`/`.kv_items()` was called last.
    /// Drives `render()`'s dispatch; the slot-specific `render_toon()`/
    /// `render_kv()` ignore it entirely. See the routing note below.
    target:            RenderTarget,
    items:             Vec<Vec<Value>>,
    fields:            Vec<String>,
    single_item:       Vec<kv::KvItem>,
    total_count:       Option<usize>,
    hints:             Vec<Hint>,
    recovery:          Vec<RecoveryHint>,
    truncate_cells_at: usize,
    is_error:          bool,
    human_content:     Option<String>,
}

/// The serialisation format for `AgentResponse::render`. Only two variants —
/// the TOON-vs-KV choice is decided by which of `.items()`/`.kv_items()` was
/// called (see `target`, below), not by a value passed to `render()`.
/// `OutputFormat` only selects text vs. JSON.
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
    /// `isError` field. Maps directly onto MCP's `CallToolResult.isError`.
    pub fn as_error(mut self) -> Self
    /// Attach a human-facing companion block (`audience: user`) for MCP
    /// callers. Optional. See 04-mcp-and-napi.md.
    pub fn human_content(mut self, text: impl Into<String>) -> Self

    pub fn render(&self, format: OutputFormat) -> String
    /// Reads the TOON slot (`items`/`fields`/`total_count`) unconditionally —
    /// not a shorthand for `render(OutputFormat::Text)`, which instead follows
    /// whichever of `.items()`/`.kv_items()` was called last. See below.
    pub fn render_toon(&self) -> String
    /// Reads the KV slot (`single_item`/`total_count`) unconditionally — the
    /// KV-path counterpart of `render_toon()`.
    pub fn render_kv(&self) -> String
    pub fn render_hints_only(&self) -> String // see 06-decisions.md: three-surface seam
    /// Builds the MCP `CallToolResult` for this response. See 04-mcp-and-napi.md.
    pub fn to_call_tool_result(&self) -> crate::mcp::CallToolResult
}
```

`render_json(&self) -> String` isn't a public method — JSON is reached via
`render(OutputFormat::Json)`, matching the rest of the crate's zero-`serde_json` default. It's
hand-built JSON text, not `serde_json::Value`. (The NAPI wrapper does expose a convenience
`renderJson(): string` — TypeScript callers shouldn't need to import an `OutputFormat` enum just
to reach the JSON path.)

The `items` (TOON) and `single_item` (KV) slots are stored independently, so the two paths never
conflict at the data level. `render(format)` dispatches on `target` — whichever of
`.items()`/`.kv_items()` was called *last* — for both text and JSON. The named shorthand methods
are stronger: `render_toon()` reads only the `items`/`fields` slot and `render_kv()` reads only
`single_item`, regardless of which setter was called last or what `target` currently is.
Concretely: `AgentResponse::new("x").items(rows, &fields).kv_items(kv_rows).render_toon()` still
renders the TOON list — it ignores `target` (which is `Kv` here) the way
`render(OutputFormat::Text)` wouldn't. Calling both `.items()` and `.kv_items()` on one builder is
a caller-side logic error to avoid, not a panic. Treat one `AgentResponse` as one output shape.

### `render_for()` and `has_human_content()` — dual CLI/agent output

The same `human_content`/audience split that powers `to_call_tool_result()`
(see [04-mcp-and-napi.md](04-mcp-and-napi.md)), available directly, for any consumer — not just
MCP:

```rust
impl AgentResponse {
    /// Render for the given audience. `Assistant` matches `render(OutputFormat::Text)`.
    /// `User` returns `human_content` if set, falling back to the same
    /// agent-oriented rendering otherwise — never empty, never a panic.
    pub fn render_for(&self, audience: Audience) -> String

    /// Whether `.human_content()` was set on this builder.
    pub fn has_human_content(&self) -> bool
}
```

Deciding *which* `Audience` applies to a given invocation — TTY detection, a CLI flag, an
environment variable — stays entirely the caller's job; that's argument-parsing/environment
territory, inside this crate's existing "no CLI framework" non-goal. michi only owns the signal
once that decision's already made.

The fallback in `render_for(Audience::User)` is a real behavior worth designing around, not just
a safety net — TOON/KV text is comma-syntax built for a model to parse, not for a human to read
comfortably. A caller intending to actually use the `User` path should call `.human_content()`
first; `has_human_content()` lets a caller downstream of wherever the response was built — a
renderer that only knows the audience, not how the response was constructed — check before
rendering, rather than discovering the fallback only by inspecting the text it gets back.
