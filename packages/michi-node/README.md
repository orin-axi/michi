# michi

TypeScript/JavaScript bindings for [`michi`](https://github.com/orin-axi/michi) — AXI
response primitives for agent-ergonomic tools. Native binary via NAPI-RS, no WASM, no
JS reimplementation of the rendering logic.

## Install

```bash
pnpm add michin
```

## Quick start

```typescript
import { renderToon } from "michin";

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

```text
issues[2]{number,title,state}:
  51815,[Bug]: Telegram plugin,open
  51812,dark mode request,open
totalCount: 8771
help[1]:
  Run `gh-axi issue view <number>` to view an issue
```

## API

| Function | Signature |
|---|---|
| `renderToon` | `(opts: JsToonOptions) => string` — TOON list rendering |
| `emptyState` | `(typeName: string) => string` — definitive empty-state block |
| `renderHints` | `(hints: string[]) => string` — `help[N]:` block |
| `truncate` | `(content: string, maxChars: number, hint: string) => string` |

Full type definitions ship in `index.d.ts`. See the
[main repo](https://github.com/orin-axi/michi) for the complete primitive set, the TOON
grammar, and the Rust API this wraps.

## Supported platforms

Prebuilt native binaries: `darwin-arm64`, `linux-x64-musl`. Building from source requires
a Rust toolchain (see [`rust-toolchain.toml`](../../rust-toolchain.toml) in the main repo).

## License

AGPL-3.0-or-later. See [`LICENSE`](LICENSE).
