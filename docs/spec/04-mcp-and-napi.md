# MCP and the NAPI/npm Boundary

> Targets MCP spec version **2025-11-25** (the current stable release). A release candidate for **2026-07-28** exists — described by its own announcement as "the largest revision since launch" — but hasn't shipped yet. Nothing in this doc should be read as covering the RC; revisit this note once it's finalized.

## `mcp` module

Always compiled — no feature gate, no new dependency in the default build. Owns exactly one thing: turning an already-built `AgentResponse` into the shape MCP's `tools/call` response expects.

```rust
// Audience lives in src/audience.rs — not MCP-specific, michi's CLI output
// path uses it too (see 03-rust-api.md's render_for()/has_human_content()).
pub struct ContentBlock {
    pub text: String,
    pub audience: Vec<Audience>,   // MCP's annotations.audience is an array
}

pub struct CallToolResult {
    pub content: Vec<ContentBlock>,
    pub is_error: bool,
    pub structured_content: String,   // JSON text; see field doc for what's actually structured
}
```

`audience` is a `Vec`, not a scalar, because MCP's real `annotations.audience` is an array — one block can target more than one audience. michi always populates exactly one element per block today, but the type matches the wire shape, so no translation is needed at the serialization boundary.

Under the `serde` feature, `ContentBlock`/`CallToolResult` don't derive `Serialize`/`Deserialize` directly onto their Rust-shaped fields — a naive derive would emit `is_error`/`structured_content` (snake_case) and a bare `audience`, neither valid MCP JSON. Each type instead converts through a private `*Wire` struct (`serde(into = "...", from = "...")`) that adds the `"type": "text"` discriminator, nests `audience` under `annotations`, renames fields to camelCase, and — for `CallToolResult` — parses `structured_content` into a real embedded JSON value rather than a double-encoded string. `Audience` itself serializes lowercase (`"assistant"`/`"user"`), matching MCP's `Role` type.

`AgentResponse::to_call_tool_result()` (see [03-rust-api.md](03-rust-api.md)) is the only intended constructor. It builds the primary `assistant`-audience block from whichever of `render_toon()`/`render_kv()` is active, an optional second `user`-audience block from `human_content()` if the caller set one, and `structured_content` from `render(OutputFormat::Json)`.

---

## NAPI npm package — `@orin-axi/michi`

A thin napi-rs wrapper around the same crate, built with the `napi` feature enabled:

- Feature-gated `Cargo.toml` with the `napi` feature
- `napi-rs` with `napi-derive` proc-macros — clean Rust in, C-ABI glue and TypeScript types out, generated at compile time
- Typed `#[napi(object)]` structs (`JsToonValue`, `JsKvItem`, ...) for the dynamic FFI boundary (cell values, recovery params) — kept as tagged Rust enums converted by hand, not `serde_json::Value`. `serde_json` does enter the tree once `napi` is enabled (for `toCallToolResult()`'s typed `structuredContent`), but the two are independent choices — `JsToonValue`/`JsKvItem` stay their own explicit shape because a `Value` would silently accept malformed input instead of matching a documented variant.
- Platform-aware binary loading via the generated `index.js`
- TypeScript fallback export for environments without the native binary
- Cross-compiled via `cargo-zigbuild`:
  - `aarch64-apple-darwin` (`darwin-arm64`)
  - `x86_64-unknown-linux-musl` (`linux-x64-musl`)

### Why `napi` v3 / the `napi6` feature

napi-rs gates capabilities behind Node-API version features. This crate targets `napi` v3 with the `napi6` feature — v3 dropped the Docker-image requirement v2 had for cross-compilation, which is the actual reason for the move (see [06-decisions.md](06-decisions.md)). michi's exports are synchronous pure functions either way, so the async machinery either feature level gates isn't something michi exercises — the choice is about build tooling, not a capability michi needs.

### The builder-boundary problem

This is the one genuinely hard part of wrapping michi in napi-rs, worth spelling out. The Rust `AgentResponse` is a **consuming** builder — every setter takes `self` by value and returns `Self`:

```rust
pub fn items(mut self, rows: Vec<Vec<Value>>, fields: &[&str]) -> Self
```

That idiom can't cross the napi boundary directly. A `#[napi]` class instance is owned by the JavaScript garbage collector; Rust only ever gets `&self` or `&mut self`, never ownership, because a live JS reference still points at the underlying struct. `pub fn items(self, ...) -> Self` simply isn't writable on a napi class.

The fix: store the consuming Rust builder in an `Option` slot, and `take()` it out on each mutation — apply the consuming method, put the result back.

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
    /// `Self` — see below for why chaining doesn't survive the NAPI boundary.
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

`Option` acts as a nullable ownership slot: `take()` moves the value out and leaves `None`, the consuming method runs, the result gets reassigned.

### Chainable setters — deliberately not implemented

Every setter on `JsAgentResponse` returns `napi::Result<()>` (void on success), not `this`. Making a setter return `this` across the napi boundary means handing back a JS-visible reference to the _same_ wrapped object from a `&mut self` method — napi-rs's `#[napi]` class methods don't support that without a more invasive change to the `Option`+`take()` pattern above (e.g. holding a JS-side handle back to `self` and threading it through every mutator's return). Given the pattern above already works and is fully tested, that wasn't worth it for a purely cosmetic JS ergonomics win. TypeScript callers use sequential statements, not method chaining:

```typescript
const r = new AgentResponse("issues");
r.items(rows, ["number", "title"]); // returns undefined, not r
r.hint("Try a broader filter"); // ditto
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

(Async `&mut self` is unsound in napi-rs — it can't enforce exclusivity across the event loop, and would require an `unsafe` marker. michi's setters are all synchronous, so this never comes up.)

### Error handling at the boundary

Any `#[napi]` function returning `napi::Result<T>` throws a JS `Error` on `Err`. Two practices apply here:

- Wrap fallible conversions (row shape mismatches, oversized inputs) in `Result` and surface them as `napi::Error::from_reason(...)`.
- Annotate exports that call into non-trivial rendering with `#[napi(catch_unwind)]`. Without it, **a Rust panic crashes the entire Node process** — there's no isolation boundary. With it, a panic becomes a thrown JS `Error` (or a rejected `Promise` for async) instead of process death.

### Numeric boundary

`total_count` is `usize` in Rust, and the NAPI boundary narrows further to a plain `i32` (`JsToonOptions.total_count`, `JsAgentResponse::total_count`), clamped non-negative (`n.max(0) as usize`) on the way in. JavaScript numbers are 64-bit floats with a max safe integer of 2^53, so `i32` is comfortably within the safe range — no need for an `i64`-as-`number` mapping or `BigInt`. Counts beyond `i32::MAX` aren't a realistic concern for agent list responses.

### Platform binary loading (`index.js`)

`@napi-rs/cli` generates `index.js` at build time. It tries a locally compiled `.node` file first (so `napi build` during development needs no reinstall), then falls back to the per-platform optional-dependency npm package matching `process.platform`/`process.arch`. On Linux it also branches on glibc vs. musl via `detect-libc`. Each platform binary ships as its own npm package under `optionalDependencies`, with `cpu`/`os` fields so package managers install only the matching one. If neither a local `.node` nor a matching optional dep is present, the require throws — no silent degradation, so the package's TypeScript fallback export should surface a clear error.

> Known pitfall: npm sometimes omits optional platform deps from `package-lock.json` when the lockfile was generated on a different architecture (npm/cli#4828). Prefer `pnpm`, or regenerate the lockfile on the target arch.

### TypeScript type generation

`index.d.ts` is **never hand-written**. The `napi-derive` macro emits type metadata and the CLI assembles the `.d.ts` at build time — function signatures, the `AgentResponse` class, `Promise<T>` for async, `T | null` for `Option<T>`. Where an inferred type is too loose (the dynamic cell-value arrays, for instance), the wrapper uses `#[napi(ts_arg_type = "...")]` / `#[napi(ts_return_type = "...")]` overrides so the published types match the contract below.

### Cross-compilation and CI

`@napi-rs/cli`'s `--cross-compile` flag selects `cargo-zigbuild` as the linker for non-native Linux/macOS targets (and `cargo-xwin` for Windows). Both shipped targets build from a single Linux runner:

```bash
cargo install cargo-zigbuild
napi build --release --cross-compile --target aarch64-apple-darwin
napi build --release --cross-compile --target x86_64-unknown-linux-musl
```

Publish CI uses a `fail-fast: false` matrix — one `.node` per target, uploaded as an artifact — then a publish job downloads everything and runs `napi prepublish`:

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - target: x86_64-unknown-linux-musl
        host: ubuntu-latest
        cross: true # triggers cargo-zigbuild
      - target: aarch64-apple-darwin
        host: macos-latest # native build (preferred for code-signing)
```

The publish job emits the per-platform packages and the main `@orin-axi/michi` package together. CI asserts the npm package version equals the crate version before publish (see [05-scope-and-quality.md](05-scope-and-quality.md)).

**Windows is a non-goal for v1.** michi's agent consumers (MCP servers, CLIs invoked by coding agents) run on Linux and macOS hosts; a `windows-x64` target would add `cargo-xwin` tooling and a third CI lane for no current consumer. Nothing blocks adding it later.

### TypeScript types (`index.d.ts`)

This is auto-generated, never hand-written — `napi build` regenerates it from the `#[napi]` attributes in `src/napi.rs` every time. Every object type is `Js`-prefixed, matching its Rust struct name (`JsToonValue`, `JsToonOptions`, `JsKvItem`, `JsRecoveryHint`, `JsContentBlock`, `JsAnnotations`, `JsCallToolResult`) — there's no unprefixed `ToonValue`/`CallToolResult`/etc. in the real output, whatever an older sketch of this section might have suggested. What's actually in `packages/michi-node/index.d.ts` today:

```typescript
/** Scalar TOON/KV cell value. Discriminate via `type`. */
export interface JsToonValue {
  type: string; // "str" | "int" | "float" | "bool" | "null"
  strVal?: string;
  intVal?: number;
  floatVal?: number;
  boolVal?: boolean;
}

export interface JsToonOptions {
  typeName: string;
  fields: Array<string>;
  /** Rows, each parallel to `fields`. Capped at MAX_ROWS rows, MAX_FIELDS values per row. */
  rows: Array<Array<JsToonValue>>;
  totalCount?: number;
  /** Required, not optional — pass an empty array for no hints. Capped at MAX_HINTS. */
  hints: Array<string>;
}

/** Throws if rows/fields/hints, or any row, exceed this module's per-call size
 * limits — protects Node's single-threaded event loop from unbounded
 * synchronous allocation on a crafted call. */
export declare function renderToon(opts: JsToonOptions): string;

/** Render a definitive empty-state TOON response: `typeName[0]{}:\ntotalCount: 0\n`.
 * No `hints` parameter over this boundary — use `appendHints()` to add one. */
export declare function emptyState(typeName: string): string;

/** Returns an empty string when hints is empty. */
export declare function renderHints(hints: Array<string>): string;

export declare function appendHints(body: string, hints: Array<string>): string;

/** Only the inline form crosses the NAPI boundary — unlike Rust's `truncate()`,
 * which returns a richer `Truncated` struct, this returns the final string directly. */
export declare function truncate(
  content: string,
  maxChars: number,
  hint: string,
): string;

export interface JsRecoveryHint {
  tool: string;
  /** No structured params over this boundary — use `AgentResponse.recoveryHint()`
   * for the common case, or the Rust API directly for typed params. */
  reason?: string;
}

export declare function renderRecovery(hints: Array<JsRecoveryHint>): string;

export interface JsAnnotations {
  /** `["assistant"]` or `["user"]` — michi has no concept of MCP's optional `priority`. */
  audience: Array<string>;
}

export interface JsContentBlock {
  /** Always `"text"` — michi only ever produces text content blocks. */
  type: string;
  text: string;
  annotations: JsAnnotations;
}

/** `structuredContent` is a real parsed value here (unlike `renderJson()`'s
 * string), since it crosses the boundary as a typed `#[napi(object)]` field,
 * not hand-built JSON text. */
export interface JsCallToolResult {
  content: Array<JsContentBlock>;
  isError: boolean;
  structuredContent: any;
}

export interface JsKvItem {
  key: string;
  value: JsToonValue;
}

/** High-level builder — mirrors the Rust `AgentResponse` API. Every mutator
 * returns `void`, not `this` — see "Chainable setters," above; callers use
 * sequential statements, not method chaining. */
export declare class AgentResponse {
  constructor(typeName: string);
  items(rows: Array<Array<JsToonValue>>, fields: Array<string>): void;
  totalCount(n: number): void;
  kvItems(items: Array<JsKvItem>): void;
  hint(hint: string): void;
  recoveryHint(tool: string, reason?: string): void;
  /** Marks this response as an error state, reflected in `renderJson()`'s `isError` field. */
  asError(): void;
  /** Attaches a `user`-audience companion block, included by `toCallToolResult()`
   * and readable via `renderFor("user")`. */
  humanContent(text: string): void;
  /** Renders for `"assistant"` or `"user"`. `"user"` falls back to the
   * agent rendering when `humanContent()` was never set — see
   * `hasHumanContent()`. Throws for any other audience string. */
  renderFor(audience: string): string;
  /** Whether `.humanContent()` was set on this builder. */
  hasHumanContent(): boolean;
  /** Reads the TOON slot unconditionally — see 03-rust-api.md. */
  renderToon(): string;
  /** Reads the KV slot unconditionally — see 03-rust-api.md. */
  renderKv(): string;
  /** Returns a JSON *string* — `{"body":...,"hints":[...],"recovery":[...],"isError":bool}`
   * — not a parsed value; kept as hand-built text for consistency with the Rust side's
   * `render(OutputFormat::Json)`, which stays `serde_json`-free (see the `mcp` module
   * section above for where this crate *does* use `serde_json`, and why). */
  renderJson(): string;
  renderHintsOnly(): string;
  toCallToolResult(): JsCallToolResult;
}
export type JsAgentResponse = AgentResponse;
```
