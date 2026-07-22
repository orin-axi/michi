# Decisions

Why things are the way they are. If a choice in the code looks arbitrary, the reasoning is
probably here.

## Open questions

Tracked by what they block, not by sequence number — "Q1/Q2/Q3" reads too easily as calendar
quarters, which these aren't.

**Open until validated — TOON vs. Markdown-KV retrieval accuracy (blocks: recommending TOON as
the default for MCP consumers)**
AXI benchmarks the token-efficiency gains at the CLI/MCP level, but not the downstream LLM
retrieval accuracy of TOON vs. Markdown-KV for list data. Before treating TOON as universally
superior for the `audience: ["assistant"]` surface, run a retrieval accuracy experiment: same
list data, TOON vs. Markdown-KV, a retrieval task, across a few model sizes. If Markdown-KV wins
on accuracy despite the higher token cost, the right call for MCP consumers may be Markdown-KV
even for lists. The crate ships either way — the open question is which format callers should
default to.

**Open until a real integration exists — crates.io publish readiness (blocks: crates.io release)**
For external Rust binaries to depend on `michi`:
(a) crates.io publish — cleanest for public consumers once the API is stable
(b) git dep with tag: `michi = { git = "https://github.com/orin-axi/michi", tag = "v0.x.y" }` —
    good for early adopters before crates.io publish
(c) private cargo registry — overhead not worth it

Recommendation: (b) during development, (a) at first stable release. crates.io publish is gated
behind at least one real consumer integration test.

**Deferred to v2 — `cli` feature scope (blocks: terminal-aware rendering)**
Reserved for terminal-aware rendering. If `--format toon` in a human terminal context should wrap
long rows or colourize the header, this is where that lives — likely a `render_terminal()`
variant consulting terminal width (`crossterm`) and applying ANSI colour to health signals and
the type header. Out of scope for v1: michi v1 targets agent consumers only, not human-readable
terminal output.

**Resolved at v0.1 — `AgentResponse` builder vs. standalone functions**
Both: `AgentResponse`/`JsAgentResponse` is exported, along with
`renderHints()`/`appendHints()`/`renderRecovery()`/`renderToon()`/`emptyState()`/`truncate()` —
the low-level functions are exported *additively*, not instead of the builder, since removing an
already-shipped export is a breaking change with no upside. Rust-only consumers reach every
primitive directly; TypeScript consumers get both the builder and the standalone functions. This
also settled the chainable-builder question: `JsAgentResponse`'s setters return `undefined`, not
`this` — see [04-mcp-and-napi.md](04-mcp-and-napi.md)'s "Chainable setters" section.

**Resolved at v0.1 — Recovery hint format**
`RetryHint.params` uses `kv::KvValue`, an existing already-tested enum, instead of
`serde_json::Value` — fixes the Rust-side type-loss concern without adding a dependency. A
related bug surfaced during this reconciliation and got fixed alongside it: independent of which
Rust-side type `params` used, `AgentResponse::render_json()`'s JSON *output* was, for a time,
stringifying every param value regardless of its `KvValue` variant (`{"seconds":"30"}` instead of
`{"seconds":30}`) — which would have defeated the point of a typed enum entirely.
`render_json()` now serializes `Int`/`Float`/`Bool` as native JSON literals, `Text` as a JSON
string, `Missing` as `null`, so both the Rust-side and JSON-output representations carry real
type information. Covered by tests in `src/response.rs` and `src/kv/mod.rs`.

---

## Why the API is shaped the way it is

This crate went through more than one draft. None of what follows is "the API used to be wrong" —
it's the reasoning behind why the current shape won, in case a future change tempts you back
toward an earlier idea that was already tried and rejected for a reason.

**`ToonOptions` bundles everything, not five positional arguments.** An early draft split
`render_toon()`'s inputs across five positional parameters plus a small options struct holding
only `max_cell_len`/`total_count`. The shipped version folds every input — `type_name`, `fields`,
`rows`, `hints` included — into `ToonOptions` itself, taken by reference. One struct, one thing to
construct.

**`append_hints` mutates in place.** It takes `out: &mut String` and appends to it, rather than
taking `body: &str` and returning a new `String`. That avoids an allocation-and-copy this crate
removes everywhere it can — `recovery::append_recovery` follows the identical pattern. The
content appended is the same either way; this is a signature choice, not a behavior change.

**`truncate()`/`truncate_inline()` take `hint: &str` as a parameter.** An early version hardcoded
`"full=true"` for every call site. Every example in this spec happening to use that exact string
doesn't mean every caller's actual escape-hatch flag is named that — taking it as a parameter
costs nothing and removes an assumption michi has no business making.

**`error::DomainError` and `error::Error` are two types, not one.** An early draft called this
type `AxiError` and made it the crate's entire error type. The shipped design splits it:
`DomainError` is the pure data/render type — what [03-rust-api.md](03-rust-api.md) documents.
`Error` is a `thiserror`-derived enum with a `Domain(DomainError)` variant alongside always-
compiled `InvalidInput`/`NotFound` variants and, behind the `pipeline` feature, execution-layer
variants (`Http`, `Timeout`, `StepFailed`) that need `#[source]`-chaining a single struct can't
express.

**`idempotency::already_done()` only checks — it doesn't render.** An early draft had one
function do both. michi owns no persistence layer, so it has no way to know on its own whether an
operation already happened — only your store does. Splitting check from render is the honest
shape: `already_done()` takes what your lookup found and classifies it; `render_already_done()`
renders regardless of how you detected the no-op.

**`IdempotencyKey::new()` takes one pre-combined string, not `(operation, stable_input)`.** A
caller that wants an operation-name-plus-input key builds it themselves
(`format!("{operation}:{stable_input}")`) before calling `new`, or uses `from_hash` for the hashed
variant. `from_hash` uses FNV-1a, not SHA-256 — idempotency keys need stability and low collision,
not cryptographic security, and `sha2` stays behind the `cache` feature only.

**`RetryConfig`'s `jitter_factor: f64`, not `jitter: bool`.** `f64` supports partial jitter, not
just full-jitter-or-none — strictly more capable at the same call-site cost. This was also the
subject of an adversarially-found bug: jitter could previously exceed `max_delay`, since nothing
capped the jittered result against it. Fixed now, and `next_retry_delay`'s four-parameter
signature (`config`, `attempt`, `jitter_seed`, `retry_after`) reflects that fix plus the
`retry_after` integration. It returns `Option<Duration>`, not a bare `Duration`, so a caller can
tell "one more attempt, with this delay" apart from "retries exhausted."

**`StatusResponse.tool_name`/`.description` are `String`, not `&'static str`.** A tool's name and
description are runtime-computed at every real call site this crate has been used from — often
built from config or a manifest. `&'static str` would force every caller to hardcode a literal or
leak an allocation just to satisfy the lifetime.

**`OutputFormat` has two variants, not three.** An early draft had a three-way
`Toon`/`Kv`/`Json` split. The shipped design decides TOON-vs-KV by which of `.items()`/
`.kv_items()` was called (see `target` in [03-rust-api.md](03-rust-api.md)'s `response` section),
not by a value passed to `render()` — so `OutputFormat` only needs to select text vs. JSON.

**`AgentResponse` has no `unsafe impl Send`/`Sync`.** Every field is an owned, non-interior-
mutable type (`String`, `Vec`, `Option<usize>`, `bool`, ...), so `Send`/`Sync` already hold
through Rust's automatic derivation. An explicit `unsafe impl` would be both redundant and a
violation of this crate's own "no `unsafe` outside the NAPI boundary" rule.

**`napi` v3 with the `napi6` feature, not v2 with `napi4`.** `napi4` was, at the time, the lowest
level exposing `ThreadsafeFunction` and the async support napi-rs needs, with Node.js 12 as the
floor. The move to v3/`napi6` happened because v3 dropped the old Docker-image cross-compilation
requirement — not because michi needed any new capability. Its exports are synchronous either
way; the choice is entirely about build tooling.

---

## Implementation notes

**`IdempotencyKey::from_hash` needs deterministic input.** For the key to be canonical across
calls with the same logical input, the bytes you hash must be produced deterministically:

- **Maps and structs:** sort keys before serializing. `serde_json::to_vec` on a `HashMap`
  produces non-deterministic key order — use `BTreeMap` or a sorted intermediate representation.
- **Floats:** avoid them in idempotency keys; representation varies by architecture. Round to a
  fixed decimal or convert to integers.
- **Timestamps:** exclude request-time fields (`created_at`, `request_id`) unless you actually
  want a per-request key rather than a per-operation one.

`from_hash` takes raw `&[u8]` — michi has no opinion on how you produce those bytes, and doesn't
itself depend on `serde_json`. The example below uses it only because JSON is a common choice for
callers already producing it elsewhere; any deterministic serialization works equally well as
long as it follows the rules above.

```rust
// Correct: BTreeMap serialises with sorted keys
let mut params: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
params.insert("project", json!("PROJ"));
params.insert("type",    json!("Task"));
let key = IdempotencyKey::from_hash("create_item", &serde_json::to_vec(&params)?);

// Incorrect: HashMap key order is non-deterministic
let params: HashMap<&str, _> = /* ... */;
let key = IdempotencyKey::from_hash("create_item", &serde_json::to_vec(&params)?);
// ^ same logical input may produce a different key on different calls
```

**Why `is_retryable_status()` excludes 500.** The default retryable set is
`{ 429, 502, 503, 504 }`. 429 and the 50x gateway errors represent transient conditions — rate
limits, upstream unavailability — where the same request will likely succeed after a delay. 500
represents a server-side bug: retrying without changing anything reproduces the same 500, and for
write operations, may duplicate side effects if the server processed the request before erroring.
Callers that genuinely need to retry 500s (some APIs use it for transient conditions) can layer
their own predicate on top:

```rust
// Default: 429, 502, 503, 504 only
if is_retryable_status(status) { ... }

// Custom: caller adds 500 for a specific API known to use it transiently
let retryable = is_retryable_status(status) || status == 500;
```

**`render_hints_only()` — the three-surface seam.** Supports MCP frameworks implementing a
three-surface pattern: one content block for the agent (`audience: ["assistant"]`), one for
display (`audience: ["user"]`), and a `structuredContent` payload for client tooling. The display
body is rendered by the framework's own Markdown/rich-text layer; the agent-facing block combines
TOON output with a `help[]` trailer. Without `render_hints_only()`, framework code that's already
rendered a display body would need to reconstruct an entire `AgentResponse` just to get a
formatted `help[]` block to append.

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

**`parse_retry_after()` format details.** Accepts both forms from RFC 7231 §7.1.3:

| Form | Example | Notes |
|---|---|---|
| Integer seconds | `"120"` | Seconds to wait from response time |
| HTTP-date | `"Wed, 21 Oct 2026 07:28:00 GMT"` | Absolute datetime, UTC only |

Returns `None` for anything malformed — callers should treat that as "use backoff only" and call
`next_retry_delay()` with `retry_after: None`. Doesn't validate that an HTTP-date is in the
future: a server returning a past date (clock skew, a bug) produces a zero or very small
duration from this function alone, but `next_retry_delay()` clamps the final result to at least
`config.initial_delay` regardless, so no extra handling is needed at the call site.

**`render_toon()`'s capacity estimate.** A heuristic, not a hard bound:

```
capacity = 60 + row_count × (field_count × 12 + 10) + hints.len() × 60
```

The `60` covers the type name, bracketed count, and brace-wrapped field list; `× 12 + 10` per row
estimates a typical short cell (an id, a short title fragment, a state word) plus its delimiter;
`hints.len() × 60` covers the trailing `help[]` block. The estimate intentionally runs slightly
high so the common case never reallocates — a wildly under-estimated row (very long untruncated
cells) may trigger one reallocation, acceptable since `max_cell_len` truncation caps the realistic
upper bound. None of these are configurable — they're tuning constants, not part of the public
contract.

**`outputSchema` is in the LLM context; `structuredContent` isn't.** Matters for token budget
planning in MCP servers using progressive disclosure or deferred tool loading. `outputSchema` on
a tool definition is part of the `tools/list` response — it *is* injected into the context window
at session start alongside `inputSchema`, adding roughly +15% on the schema component (an
approximate, workload-dependent figure). It's a one-time cost that amortizes across the session.
`structuredContent` in a tool result is client-consumed only and never enters the context window.

This matters for deferred tool loading specifically: adding `outputSchema` to tools that aren't
loaded yet adds schema tokens before those tools are even called, defeating the point of
deferral. Only define `outputSchema` on always-loaded tools, or exclude it from `tools/list` for
deferred ones and supply it only when the tool is actually invoked. If your MCP framework
auto-registers `outputSchema` from tool definitions, make sure it respects that boundary.

**Build-time P4 — pre-computed aggregates at CI time.** AXI Principle 4 usually gets discussed as
a runtime convention: always return `totalCount`, pre-filter before responding, summarize instead
of dumping raw data. The same principle applies at build time, and that's where the bigger wins
are.

The pattern: any deterministic computation a tool needs repeatedly is a candidate for
pre-computing in CI instead of running it per request.

Canonical example — a design system component library. At build time, a bundler plugin scans
every component's definitions, resolves references, and writes a static lookup index shipped with
the MCP server package. When an agent calls `searchComponents()`, the handler reads that static
structure — no scanning, no resolution, no index construction per request. The agent pays zero
turn cost for the computation; the CI pipeline pays it once.

| Computation | Runtime | Build time |
|---|---|---|
| Filtering by user input | ✓ | — |
| Counting available items | Runtime (`totalCount` field) | Build time (pre-count, embed in manifest) |
| Resolving references between definitions | — | ✓ (bundler plugin) |
| Indexing for search / BM25 | — | ✓ (write index to disk at build) |
| Building dependency graphs | — | ✓ (static graph serialised to JSON) |
| Generating tool output schemas | — | ✓ (schema library `toJsonSchema()` at registration) |

This is a design decision, not a code pattern — it shows up during architecture planning, not
implementation. Ask: does this computation change per request, or is it stable across many
requests? Stable computation belongs at build time.
