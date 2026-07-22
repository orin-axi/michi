# michi spec

The technical reference for the `michi` crate — what it does, how it's shaped, and why.

## Reading order

| Doc | Covers |
|---|---|
| [01-overview-and-setup.md](01-overview-and-setup.md) | What michi is, why it exists, what's deliberately out of scope, who consumes it, `Cargo.toml`, crate layout |
| [02-toon-format.md](02-toon-format.md) | The TOON grammar — the agent-facing list format |
| [03-rust-api.md](03-rust-api.md) | Every module: `toon`, `kv`, `hints`, `truncate`, `empty`, `error`, `idempotency`, `resilience`, `status`, `recovery`, `response` |
| [04-mcp-and-napi.md](04-mcp-and-napi.md) | The `mcp` module, the NAPI/npm boundary, and the builder-across-FFI problem |
| [05-scope-and-quality.md](05-scope-and-quality.md) | What's formalized vs. what stays in your app, feature flags, versioning, performance, testing |
| [06-decisions.md](06-decisions.md) | Open questions and the reasoning behind everything that isn't obvious from the code |

New to the crate? Read 01 → 02 → 03 in order — that's the whole mental model. 04–06 are reference, read as needed.

## Known gaps

- **TOON vs. Markdown-KV retrieval accuracy hasn't been measured.** The token-count win is real; whether it holds up on retrieval accuracy across model sizes is untested. See [06-decisions.md](06-decisions.md).
- **No real consumer depends on michi yet.** crates.io publish is explicitly gated on that happening first.
- **The `cli` feature is a stub.** Reserved for terminal-aware rendering, not implemented.
- **Plan 2 (the async execution layer) is mostly stubs.** `pipeline` has a real, tested data type and `render()`; the actual executor, `fuzzy`, `cache`, and the resilience `circuit`/`policy` modules are one-line stub comments. This spec covers Plan 1, the pure-primitives crate, only.

## Companion docs

- [`../../PRINCIPLES.md`](../../PRINCIPLES.md) — what belongs in michi and why, plus how work here gets done
- [`../../ARCHITECTURE.md`](../../ARCHITECTURE.md) — the design in prose, one level up from this spec's API detail
- [`../../CONTRIBUTING.md`](../../CONTRIBUTING.md) — how to actually send a PR
