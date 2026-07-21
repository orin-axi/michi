# michi (道)

AXI response primitives for agent-ergonomic tools — TOON lists, key-value blocks,
truncation, structured errors, status, and `help[]` hints, in one small, pure-computation
crate.

`michi` is the primitive layer of the **orin-axi** suite. It doesn't know about your
protocol, your CLI framework, or your async runtime — it takes structured data in and
returns token-efficient, agent-readable strings out. Four tools in the suite (Monokl,
Firkin, Lumen, Pulse) build on it instead of re-implementing this formatting themselves.

Available from Rust directly, or from TypeScript via the `@orin-axi/michi` npm package.

## Why

Agents pay for every token they read. A JSON list of even modest size burns tokens on
repeated field names and punctuation that a human reader would never notice and an LLM
doesn't need restated per row. TOON (Token-Optimized Object Notation) states field names
once, in the header:

```json
[
  { "number": 51815, "title": "[Bug]: Telegram plugin", "state": "open" },
  { "number": 51812, "title": "dark mode request", "state": "open" }
]
```

```text
issues[2]{number,title,state}:
  51815,[Bug]: Telegram plugin,open
  51812,dark mode request,open
```

Same information, fewer tokens. Values only get quoted when they actually contain a
comma, quote, or newline — no wasted punctuation on the common case.

## Quick start

```rust
use michi::toon::{self, ToonOptions, Value};

let opts = ToonOptions {
    type_name: "issues".to_string(),
    fields: vec!["number".to_string(), "title".to_string(), "state".to_string()],
    rows: vec![
        vec![Value::Int(51815), Value::Str("[Bug]: Telegram plugin".to_string()), Value::Str("open".to_string())],
        vec![Value::Int(51812), Value::Str("dark mode request".to_string()), Value::Str("open".to_string())],
    ],
    total_count: Some(8771),
    hints: vec!["Run `gh-axi issue view <number>` to view an issue".to_string()],
};

print!("{}", toon::render_toon(&opts));
```

```typescript
import { renderToon } from "@orin-axi/michi";

const out = renderToon({
  typeName: "issues",
  fields: ["number", "title", "state"],
  rows: [
    [{ type: "int", intVal: 51815 }, { type: "str", strVal: "[Bug]: Telegram plugin" }, { type: "str", strVal: "open" }],
    [{ type: "int", intVal: 51812 }, { type: "str", strVal: "dark mode request" }, { type: "str", strVal: "open" }],
  ],
  totalCount: 8771,
  hints: ["Run `gh-axi issue view <number>` to view an issue"],
});

process.stdout.write(out);
```

Both produce (verbatim, checked against the actual renderer):

```text
issues[2]{number,title,state}:
  51815,[Bug]: Telegram plugin,open
  51812,dark mode request,open
totalCount: 8771
help[1]:
  Run `gh-axi issue view <number>` to view an issue
```

The Rust struct-literal shape is deliberately explicit rather than a fluent builder —
`ToonOptions` is one struct, easy to construct from whatever data you already have. A
higher-level `toon::list(type_name, items)` convenience API is also available (behind the
`serde` feature) for building `ToonOptions` directly from a slice of `Serialize`-able
structs, inferring `fields` and `rows` from the serialized shape.

## Install

```toml
[dependencies]
michi = "0.1"
```

```bash
pnpm add @orin-axi/michi     # or npm install / yarn add
```

Default features add zero runtime dependencies — no tokio, no async runtime, nothing
beyond `thiserror`. You opt into more as you need it (see below).

## What's in the box

| Module | Purpose |
|---|---|
| `toon` | TOON list rendering — the token-optimized agent list format |
| `kv` | Key-value single-item rendering |
| `hints` | `help[]` contextual next-step blocks |
| `truncate` | Token-safe content truncation, always on char boundaries |
| `empty` | Definitive empty-state responses (`count: 0`, never silent) |
| `error` | Unified `michi::Error` with agent-renderable output + retry classification |
| `idempotency` | Idempotency keys and already-done detection |
| `resilience` | Retry config, backoff delay calculation, `Retry-After` parsing |
| `status` | Health/status response rendering |
| `recovery` | Structured recovery hints for error responses |
| `response` | `AgentResponse` builder — composes all of the above |
| `pipeline` | Pipeline data model + rendering (execution lands in a later release) |

## Feature flags

| Feature | Adds | Why you'd enable it |
|---|---|---|
| `pipeline` | tokio, async execution types | running actual pipelines, not just rendering their state |
| `fuzzy` | nucleo-matcher | fuzzy-resolving ambiguous agent input |
| `cache` | moka, sha2 | two-tier caching with tag invalidation |
| `cli` | indicatif, inquire, crossterm, ctrlc | building a CLI surface on top of michi |
| `mcp` | (pure logic, no extra deps) | building an MCP server surface |
| `full` | everything above except `napi` | test/bench harnesses |

The default build (no features) is the one most consumers want: pure rendering, no
runtime, no surprises in your dependency tree.

## Docs

- [`docs/00-overview.md`](docs/00-overview.md) — what this is and where it fits in the suite
- [`docs/01-spec.md`](docs/01-spec.md) — full API reference and the TOON grammar
- [`docs/superpowers/specs/2026-07-03-michi-design.md`](docs/superpowers/specs/2026-07-03-michi-design.md) — architecture decisions

## Development

```bash
just test    # rust + node tests
just check   # fmt + clippy + deny + typos
just bench   # divan benchmarks
```

See [`CLAUDE.md`](CLAUDE.md) for the full contributor ground rules.

## License

AGPL-3.0-or-later. See [`LICENSE`](LICENSE).
