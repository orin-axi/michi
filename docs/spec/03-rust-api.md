# Rust API

## Crate Root (`michi`)

`michi` is a unified facade crate that re-exports all primitives from its workspace sub-crates (`michi-truncate`, `michi-resilience`, `michi-toon`, `michi-core`):

```rust
pub use michi_core::audience::Audience;
pub use michi_core::empty::{empty_state, empty_state_with_hints};
pub use michi_core::error::{DomainError, Error, ErrorClass, ErrorCode, Sensitive};
pub use michi_core::hints::{append_hints, render_hints, Hint};
pub use michi_core::kv::render_kv;
pub use michi_core::mcp::{CallToolResult, ContentBlock};
pub use michi_core::recovery::RecoveryHint;
pub use michi_core::response::{AgentResponse, OutputFormat};
pub use michi_core::status::StatusResponse;

pub use michi_resilience::{already_done, next_retry_delay, parse_retry_after, render_already_done, AlreadyDone, IdempotencyKey, RetryConfig};

pub use michi_toon::{list, render_toon, ToonOptions, Value};
pub use michi_truncate::{truncate, truncate_inline, Truncated};
```

All sub-crates compile to `wasm32-unknown-unknown` and `wasm32-wasip1` without OS dependencies.

## `michi-toon` — List Rendering

`michi-toon` formats tabular lists using Token-Optimized Object Notation (**AXI P1**). Field headers are declared once, and small cell strings ($\le 24$ bytes) are stored on the stack using `compact_str::CompactString`.

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
    pub fn total_count(mut self, total: Option<usize>) -> Self;
    pub fn hints(mut self, hints: Vec<String>) -> Self;
    pub fn max_cell_len(mut self, len: usize) -> Self;
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
```

### Direct Serde Streaming (`ToonSerializer`)

When the `serde` feature is enabled, `michi::toon::list("type", &slice)` streams any `T: serde::Serialize` slice directly into `ToonOptions` without allocating intermediate `serde_json::Value` trees.

```rust
#[cfg(feature = "serde")]
pub fn list<T: serde::Serialize>(type_name: impl Into<String>, items: &[T]) -> ToonOptions;
```

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
    pub fn hint(mut self, hint: impl Into<String>) -> Self;
    pub fn recovery(mut self, r: RecoveryHint) -> Self;
    pub fn render(&self) -> String;
    pub fn render_github_annotation(&self) -> String;
}
```

- `render()` outputs an agent-readable key-value block with exit code and hints.
- `render_github_annotation()` outputs a GitHub Actions workflow annotation (`::error title=code::message`).
- When the `miette` feature is enabled, `DomainError` implements `miette::Diagnostic` for colorized CLI error card rendering.
- When the `schemars` feature is enabled, `DomainError`, `StatusResponse`, and `ToonOptions` derive `JsonSchema`.

### `AgentResponse` Builder

`AgentResponse` composes lists, key-value single items, hints, and audience routing:

```rust
pub struct AgentResponse { ... }

impl AgentResponse {
    pub fn new(type_name: impl Into<String>) -> Self;
    pub fn items(mut self, rows: Vec<Vec<Value>>, fields: &[&str]) -> Self;
    pub fn kv_items(mut self, items: Vec<KvItem>) -> Self;
    pub fn total_count(mut self, total: usize) -> Self;
    pub fn hint(mut self, hint: impl Into<String>) -> Self;
    pub fn human_content(mut self, content: impl Into<String>) -> Self;
    pub fn render(&self, format: OutputFormat) -> String;
    pub fn render_for(&self, audience: Audience) -> String;
    pub fn to_call_tool_result(&self) -> CallToolResult;
}
```

## `michi-truncate` — Char-Boundary Truncation

```rust
pub struct Truncated {
    pub content: String,
    pub original_len: usize,
    pub was_truncated: bool,
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

pub fn next_retry_delay(config: &RetryConfig, attempt: u32, jitter_seed: f64, retry_after: Option<Duration>) -> Option<Duration>;
pub fn parse_retry_after(header_value: &str) -> Option<Duration>;
pub fn already_done(stored: Option<String>) -> AlreadyDone;
pub fn render_already_done(operation: &str, summary: &str, hints: &[String]) -> String;
```

Calculates exponential backoff with jitter and parses RFC 7231 `Retry-After` HTTP header values.
