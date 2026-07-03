# michi — Overview

> orin-axi · Draft · June 2026

---

## What it is

`michi` is a Rust crate of pure agentic response primitives — the formatting and signalling
conventions that make tools ergonomic for LLMs regardless of protocol or language. It is the
primitive layer of the **orin-axi** suite, encoding the [AXI](https://axi.md) (Agent eXperience
Interface) design principles as typed, tested Rust.

AXI is a set of ten design principles for agent-ergonomic tooling that treats token budget as a
first-class constraint. Of the ten, michi encodes **seven** directly: TOON list rendering (P1),
content truncation (P3), pre-computed aggregates (P4), definitive empty states (P5), structured
errors with exit codes (P6), content-first status responses (P8), and contextual disclosure via
`help[]` blocks (P9). It also ships supporting idempotency and retry-delay primitives. The
remaining three principles (P2, P7, P10) depend on integration that michi deliberately excludes.

The crate is intentionally narrow: **no protocol knowledge, no async runtime, no CLI framework**.
Pure computation — data in, strings and types out. TypeScript consumers reach it via the NAPI npm
wrapper `michi`; Rust consumers take a direct crates.io or git dependency.

---

## Quick start

The same primitives, from either language. (Illustrative — see `docs/01-spec.md` for the precise
API.)

```rust
use michi::toon;

#[derive(serde::Serialize)]
struct Issue {
    number: u64,
    title: String,
    state: String,
}

let issues = vec![
    Issue { number: 51815, title: "[Bug]: Telegram plugin".into(), state: "open".into() },
    Issue { number: 51812, title: "dark mode request".into(), state: "open".into() },
];

// TOON list, an inline total count, and a next-step hint.
let response = toon::list("issues", &issues)
    .total(8771)
    .hint("gh-axi issue view <number>", "view an issue")
    .render();

print!("{response}");
```

```typescript
import { toon } from "michi";

const issues = [
  { number: 51815, title: "[Bug]: Telegram plugin", state: "open" },
  { number: 51812, title: "dark mode request", state: "open" },
];

// Identical primitives, via the NAPI wrapper.
const response = toon
  .list("issues", issues)
  .total(8771)
  .hint("gh-axi issue view <number>", "view an issue")
  .render();

process.stdout.write(response);
```

Both produce the same token-efficient output:

```text
count: 2 of 8771 total
issues[2]{number,title,state}:
  51815,"[Bug]: Telegram plugin",open
  51812,dark mode request,open
help[1]:
  Run `gh-axi issue view <number>` to view an issue
```

---

## Name

**michi** — from 道 (Japanese: path, way, road).

The character 道 is the root of *dō* in martial arts and traditional arts: judo, kendo, aikido,
shodō, chadō. It means the path itself — and the act of travelling it with intention. In the
context of this crate, every `help[]` block is michi: a marker that tells an agent which path to
take next, with what parameters.

Fits the suite's naming aesthetic: short real words borrowed from a specific domain, with a
metaphor that earns its place.

| Name      | Origin          | Meaning                       |
| --------- | --------------- | ----------------------------- |
| Lumen     | Latin           | unit of luminous flux; light  |
| Firkin    | Middle English  | small barrel; a measured unit |
| Monokl    | Slavic / French | monocle; single-eye focus     |
| Pulse     | Latin           | vital sign; rhythmic signal   |
| **michi** | Japanese (道)   | path, way, road               |

---

## Organisation

GitHub org: **orin-axi** (`github.com/orin-axi`)

> "AXI" = Agent eXperience Interface. The orin-axi org is the umbrella for tools designed to serve
> both human developers and AI agents as first-class users. `michi` is the primitive layer — the
> shared vocabulary of agent responses that all other tools in the suite can build on.

---

## Where michi fits in the suite

michi sits below every other tool in the orin-axi suite as the shared primitive layer. Each
agent-facing tool imports it rather than re-implementing AXI formatting.

| Tool      | What it does                               | Uses michi for           |
| --------- | ------------------------------------------ | ------------------------ |
| **michi** | AXI response primitives — the shared layer | — (this crate)           |
| Monokl    | AST-based semantic code search             | TOON list output         |
| Firkin    | Barrel / index file generator              | Structured error output  |
| Lumen     | Coding-session analysis + ambient context  | Status + hint primitives |
| Pulse     | Code-health intelligence                   | TOON list output         |

Non-agentic infrastructure tools (build scripts, formatters, etc.) never encounter michi — the
package boundary enforces the separation.

---

## AXI principles encoded

| Principle                           | Module       | What michi provides                               | Status  |
| ----------------------------------- | ------------ | ------------------------------------------------- | ------- |
| P1 — Token-efficient output         | `toon`       | TOON list rendering (~40% fewer tokens than JSON) | Encoded |
| P2 — Minimal default schemas        | `toon`, `kv` | Compact rendering; field names stated once        | Caller  |
| P3 — Content truncation             | `truncate`   | Size-bounded fields with `--full` escape hatch    | Encoded |
| P4 — Pre-computed aggregates        | `toon`, `kv` | Inline `totalCount` on every list and item        | Encoded |
| P5 — Definitive empty states        | `empty`      | Explicit `count: 0` message — never silent        | Encoded |
| P6 — Structured errors & exit codes | `error`      | Typed TOON errors on stdout; clean exit codes     | Encoded |
| P7 — Ambient context                | `status`     | `StatusResponse` payload for session dashboards   | Caller  |
| P8 — Content first                  | `status`     | Live-state response for no-arg invocations        | Encoded |
| P9 — Contextual disclosure          | `hints`      | `help[]` blocks of concrete next-step templates   | Encoded |
| P10 — Consistent way to get help    | —            | Per-subcommand `--help` reference contract        | Caller  |

michi encodes **seven** principles directly — P1, P3, P4, P5, P6, P8, and P9 — as typed, tested
Rust, alongside supporting idempotency and retry-delay primitives (`idempotency`) that underpin
P6's robustness guarantees and make combined operations safe to retry.

The remaining three are the **caller's responsibility**, because each requires integration michi
deliberately omits:

- **P2 — Minimal default schemas** — michi renders any schema compactly, but the caller chooses
  which fields to expose by default and which to gate behind a `--fields` flag.
- **P7 — Ambient context** — michi produces the `StatusResponse` payload, but installing it into
  per-session hooks or a plugin system requires a CLI/agent harness.
- **P10 — Consistent way to get help** — routing per-subcommand `--help` is a CLI-framework
  concern, outside a pure primitives crate.

---

## Namespaces

| Surface                | Name             |
| ---------------------- | ---------------- |
| Rust crate (crates.io) | `michi`          |
| npm package            | `michi`          |
| Rust module path       | `michi::`        |
| GitHub repo            | `orin-axi/michi` |

---

## License

AGPL v3 (`AGPL-3.0-or-later`).

Rationale: viral copyleft is well-suited to CLI/library tools embedded in pipelines. Creates
natural friction against agentic services wrapping michi commercially without contributing back. A
CLA or contributor copyright assignment clause per repo preserves future dual-licensing
optionality.

---

## Publishing targets

- **crates.io** — public, `michi`
- **npmjs.com** — public, `michi` (NAPI wrapper)
- NAPI binaries cross-compiled via `cargo-zigbuild`:
  - `darwin-arm64`
  - `linux-x64-musl`

---

## Documents in this repo

| File                      | Contents                                              |
| ------------------------- | ----------------------------------------------------- |
| `docs/00-overview.md`     | This file — project identity, naming, suite context   |
| `docs/01-spec.md`         | Full technical specification — modules, API, grammar  |
| `docs/projects/01-mvp.md` | MVP implementation plan — agent sessions, constraints |

---

## Open questions

See `docs/01-spec.md` § Open questions for the five ADR items that need resolution before or during
the MVP build (TOON vs KV retrieval accuracy, consumer strategy, `cli` feature scope, NAPI export
surface, recovery hint typing).
