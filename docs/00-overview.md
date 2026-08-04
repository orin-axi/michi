# michi — Overview

`michi` is a Rust workspace of pure agentic response primitives — the formatting and signaling conventions that make tools ergonomic for LLMs regardless of protocol or language. It forms the primitive layer of the **orin-axi** suite.

AXI (Agent eXperience Interface) treats token budget and prompt clarity as first-class engineering constraints.

`michi` encodes seven core principles directly: TOON list rendering (P1), content truncation (P3), pre-computed aggregates (P4), definitive empty states (P5), structured errors with exit codes (P6), content-first status responses (P8), and contextual disclosure via `help[]` blocks (P9).

The workspace is intentionally narrow: **no protocol knowledge, no async runtime, no CLI framework**. Pure computation — structured data in, agent-readable strings out.

## Quick Start

### Rust

```rust
use michi::toon::{render_toon, ToonOptions, Value};

let opts = ToonOptions::new(
    "issues",
    vec!["number".to_string(), "title".to_string(), "state".to_string()],
    vec![
        vec![Value::Int(51815), Value::from("[Bug]: Telegram plugin"), Value::from("open")],
        vec![Value::Int(51812), Value::from("dark mode request"), Value::from("open")],
    ],
)
.total_count(Some(8771))
.hints(vec!["Run `gh-axi issue view <number>` to view an issue".to_string()]);

print!("{}", render_toon(&opts));
```

### TypeScript / Node.js

```typescript
import { renderToon } from "@orin-axi/michi";

const out = renderToon({
  typeName: "issues",
  fields: ["number", "title", "state"],
  rows: [
    [
      { type: "int", intVal: 51815 },
      { type: "str", strVal: "[Bug]: Telegram plugin" },
      { type: "str", strVal: "open" },
    ],
    [
      { type: "int", intVal: 51812 },
      { type: "str", strVal: "dark mode request" },
      { type: "str", strVal: "open" },
    ],
  ],
  totalCount: 8771,
  hints: ["Run `gh-axi issue view <number>` to view an issue"],
});

process.stdout.write(out);
```

### Output

```text
issues[2]{number,title,state}:
  51815,[Bug]: Telegram plugin,open
  51812,dark mode request,open
totalCount: 8771
help[1]:
  Run `gh-axi issue view <number>` to view an issue
```

## Name

**michi** — from 道 (Japanese: path, way, road).

In the context of AXI tools, every `help[]` block is a michi signal: a clear marker indicating which path an agent should take next and with what parameters.

| Name      | Origin          | Meaning                      |
| --------- | --------------- | ---------------------------- |
| Monokl    | Slavic / French | Monocle; single-eye focus    |
| Firkin    | Middle English  | Small barrel; measured unit  |
| Lumen     | Latin           | Unit of luminous flux; light |
| Pulse     | Latin           | Vital sign; rhythmic signal  |
| **michi** | Japanese (道)   | Path, way, road              |

## AXI Principles Encoded

| Principle | Module / Crate | Purpose |
| --- | --- | --- |
| P1 — Token-efficient output | `michi-toon` | TOON list rendering (~40% fewer tokens than JSON) |
| P2 — Minimal default schemas | `michi-toon`, `michi-core::kv` | Compact tabular rendering; headers stated once |
| P3 — Content truncation | `michi-truncate` | Unicode char-boundary safe content truncation |
| P4 — Pre-computed aggregates | `michi-toon` | Inline `totalCount` on list headers |
| P5 — Definitive empty states | `michi-core::empty` | Explicit `count: 0` messages — never silent |
| P6 — Structured errors & exit codes | `michi-core::error` | Typed errors with status labels, exit code 1, and hints |
| P7 — Ambient context | `michi-core::status` | `StatusResponse` payloads for tool status |
| P8 — Content first | `michi-core::status` | Direct live-state responses |
| P9 — Contextual disclosure | `michi-core::hints` | `help[]` blocks of concrete next-step templates |

## Packages and Namespaces

| Surface           | Identifier                                                       |
| ----------------- | ---------------------------------------------------------------- |
| Rust crate facade | `michi`                                                          |
| Rust sub-crates   | `michi-truncate`, `michi-resilience`, `michi-toon`, `michi-core` |
| npm package       | `@orin-axi/michi`                                                |
| GitHub repository | `orin-axi/michi`                                                 |
