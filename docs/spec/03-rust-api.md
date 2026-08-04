# Rust API

## Crate Root (`michi`)

`michi` is a unified facade crate. It re-exports sub-crates as modules and star-exports `michi-core` directly so all core types are available at the root:

```rust
pub use michi_core::*;            // DomainError, AgentResponse, KvItem, … at michi::*
pub use michi_resilience as resilience;   // michi::resilience::RetryConfig, …
pub use michi_toon as toon;              // michi::toon::ToonOptions, michi::toon::list(), …
pub use michi_truncate as truncate;      // michi::truncate::truncate(), …
```

`michi_resilience::*` is also re-exported from `michi_core`, so `michi::RetryConfig`, `michi::already_done`, etc. are available at the root without the `resilience::` prefix.

All sub-crates compile for `wasm32-unknown-unknown` and `wasm32-wasip1` without OS dependencies.

## `michi-toon` — List Rendering

`michi-toon` formats tabular lists using Token-Optimized Object Notation (**AXI P1**). Small cell strings (≤ 24 bytes) are stack-inlined via `compact_str::CompactString`.

```rust
#[non_exhaustive]
pub struct ToonOptions {
    pub type_name: String,
    pub fields: Vec<String>,
    pub rows: Vec<Vec<Value>>,
    pub total_count: Option<usize>,
    pub hints: Vec<String>,
    pub max_cell_len: usize,
}

impl ToonOptions {
    pub fn new(type_name: impl Into<String>, fields: Vec<String>, rows: Vec<Vec<Value>>) -> Self;
    pub fn total_count(self, total: Option<usize>) -> Self;
    pub fn hints(self, hints: Vec<String>) -> Self;
    pub fn max_cell_len(self, len: usize) -> Self;
    /// Validate structural invariants before rendering.
    /// Returns Err if type_name or any field name contains structural chars,
    /// or if any row's length differs from fields.len().
    pub fn validate(&self) -> Result<(), ToonError>;
}

pub enum Value {
    Str(compact_str::CompactString),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

impl From<&str> for Value { ... }
impl From<String> for Value { ... }
impl From<i64> for Value { ... }
impl From<f64> for Value { ... }
impl From<bool> for Value { ... }

#[non_exhaustive]
pub enum ToonError {
    InvalidTypeName { name: String },
    InvalidFieldName { name: String },
    RowLengthMismatch { row_index: usize, expected: usize, actual: usize },
    InvalidItem { row_index: usize, reason: String },
}
```

### Serde Integration (`list()`)

When the `serde` feature is enabled, `toon::list()` serializes any `T: serde::Serialize` slice to a rendered TOON string:

```rust
#[cfg(feature = "serde")]
pub fn list<T: serde::Serialize>(
    type_name: impl Into<String>,
    items: &[T],
) -> Result<String, ToonError>;
```

Items are serialized via `serde_json::to_value()` and must produce JSON objects (structs or maps). Non-object items and structural chars in `type_name` or field names return `Err(ToonError)`.

## `michi-core` — High-Level Response Compositor

### `DomainError`

`DomainError` provides classified domain error responses (**AXI P6**):

```rust
#[non_exhaustive]
pub struct DomainError {
    pub code: ErrorCode,
    pub message: String,
    pub hints: Vec<Hint>,
    pub recovery: Option<RecoveryHint>,
    pub retryable: bool,
    pub retry_after: Option<Duration>,
}

impl DomainError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self;
    pub fn hint(self, hint: impl Into<String>) -> Self;
    pub fn recovery(self, r: RecoveryHint) -> Self;
    pub fn retryable(self, retryable: bool) -> Self;
    pub fn retry_after(self, d: Duration) -> Self;
    pub fn render(&self) -> String;
    pub fn render_github_annotation(&self) -> String;
    pub fn render_json(&self) -> String;
    pub fn to_call_tool_result(&self) -> CallToolResult;
}
```

- `render()` outputs an agent-readable key-value block with exit code, hints, and recovery.
- `render_github_annotation()` outputs a GitHub Actions annotation (`::error title=code::message`). Newlines and CRs are percent-encoded.
- `render_json()` outputs a JSON object: `{"isError":true,"error":"...","message":"...","retryable":bool,"hints":[...],"recovery":null|{...}}`.
- `to_call_tool_result()` builds an MCP `CallToolResult` with `is_error:true` and the rendered JSON as `structured_content`.
- When the `schemars` feature is enabled, `DomainError`, `StatusResponse`, and `ToonOptions` derive `JsonSchema`.

### `AgentResponse` Builder

`AgentResponse` composes lists, key-value single items, hints, and audience routing:

```rust
pub struct AgentResponse { ... }

impl AgentResponse {
    pub fn new(type_name: impl Into<String>) -> Self;
    pub fn items(&mut self, rows: Vec<Vec<Value>>, fields: &[&str]);
    pub fn kv_items(&mut self, items: Vec<KvItem>);
    pub fn total_count(&mut self, total: usize);
    pub fn hint(&mut self, hint: impl Into<String>);
    pub fn recovery_hint(&mut self, r: RecoveryHint);
    pub fn human_content(&mut self, content: impl Into<String>);
    pub fn as_error(&mut self);
    pub fn render_toon(&self) -> String;
    pub fn render_kv(&self) -> String;
    pub fn render(&self) -> String;
    pub fn render_for(&self, audience: Audience) -> String;
    pub fn render_json(&self) -> String;
    pub fn has_human_content(&self) -> bool;
    pub fn to_call_tool_result(&self) -> CallToolResult;
}
```

### Idempotency

```rust
// In michi-core::idempotency (also at michi::PartialSuccess, michi::FailedOp)
pub struct PartialSuccess {
    pub completed: Vec<String>,
    pub failed: Vec<FailedOp>,
    pub skipped: Vec<String>,
}

impl PartialSuccess {
    pub fn render(&self) -> String;
    pub fn exit_code(&self) -> i32;   // 0 = no failures, 1 = any failed
}

pub struct FailedOp {
    pub operation: String,
    pub reason: String,
    pub recovery: Option<RecoveryHint>,
}

// In michi-resilience (also at michi::already_done, michi::AlreadyDone)
pub enum AlreadyDone { Yes { result: String }, No }
pub fn already_done(stored: Option<String>) -> AlreadyDone;
pub fn render_already_done(operation: &str, summary: &str, hints: &[String]) -> String;
pub struct IdempotencyKey(pub String);
impl IdempotencyKey {
    pub fn new(s: impl Into<String>) -> Self;
    pub fn from_hash(operation: &str, data: &[u8]) -> Self;
}
```

## `michi-truncate` — Char-Boundary Truncation

```rust
pub struct Truncated {
    pub content: String,
    pub original_len: usize,
    pub was_truncated: bool,
    /// The signal text actually embedded in content (None when not truncated
    /// or when max_chars was too small to fit any signal).
    pub signal: Option<String>,
}

pub fn truncate(content: &str, max_chars: usize, hint: &str) -> Truncated;
pub fn truncate_inline(content: &str, max_chars: usize, hint: &str) -> String;
```

Truncates strings safely at Unicode scalar boundaries (`floor_char_boundary`) without splitting UTF-8 code points.

## `michi-resilience` — Retry & Idempotency Primitives

```rust
pub struct RetryConfig {
    pub max_retries: u32,
    pub base_delay: Duration,
    pub max_delay: Duration,
    pub jitter_factor: f64,
}

impl RetryConfig {
    /// Normalizing constructor — clamps out-of-range values.
    pub fn new(max_retries: u32, base_delay: Duration, max_delay: Duration, jitter_factor: f64) -> Self;
    /// Strict constructor — returns Err for out-of-range values.
    pub fn try_new(...) -> Result<Self, RetryConfigError>;
}

pub fn next_retry_delay(
    config: &RetryConfig,
    attempt: u32,
    jitter_seed: f64,          // per-call RNG value in [0.0, 1.0]
    retry_after: Option<Duration>,
) -> Option<Duration>;

pub fn parse_retry_after(header_value: &str) -> Option<Duration>;
pub fn is_retryable_status(status: u16) -> bool;
```

Calculates exponential backoff with caller-supplied jitter and parses RFC 7231 `Retry-After` HTTP header values (delta-seconds and HTTP-date formats).
