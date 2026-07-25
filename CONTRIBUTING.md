# Contributing to michi

Thanks for taking a look. This is a small, deliberately narrow crate — most contributions will be a bug fix, a new primitive, or filling in a Plan 2 stub. Here's how to get moving.

If you're proposing a new primitive, read [`PRINCIPLES.md`](PRINCIPLES.md) first — it's the checklist for what belongs in michi and why, so you're not guessing at unwritten rules.

## Setup

```bash
git clone git@github.com:orin-axi/michi.git
cd michi
just build
just test
```

You'll need a stable Rust toolchain (see `rust-toolchain.toml`) and, for the npm package, `pnpm` 9+. `just` is the task runner — run `just --list` for everything available.

## Before you open a PR

```bash
just check   # fmt-check + clippy (pedantic, all features) + cargo-deny + typos + markdown format check
just test    # cargo nextest (rust) + node test suite
```

Both need to pass. CI runs the same checks, so this just saves you a round trip.

## Ground rules

The short version — full detail in [`CLAUDE.md`](CLAUDE.md):

- No `unwrap()`/`expect()`/`panic!` in library code (tests may use `.expect("message")`)
- No `unsafe` outside the NAPI boundary (`src/napi.rs`)
- Every `pub` item gets a doc comment
- Strings that get built up are pre-allocated with `String::with_capacity`
- Truncation always respects char boundaries — never byte-slice blindly
- Tests run via `cargo nextest`, not `cargo test`; benchmarks are `divan`, not `criterion`
- Markdown prose isn't hard-wrapped — one line per paragraph, enforced by `just fmt-md` (oxfmt, `proseWrap: never`)

These aren't arbitrary — they're what keeps a crate meant to sit under four other tools predictable to depend on. If a rule is actively fighting you on something, say so in the PR rather than working around it quietly; the lint config has already been tuned once based on exactly that kind of feedback.

## Workflow

We use test-driven development for fixes and new primitives: write a test that fails for the right reason first, then make it pass. For anything touching more than one file, a quick look at [`ARCHITECTURE.md`](ARCHITECTURE.md) will save you some guessing about which side of the always-compiled / feature-gated line something belongs on.

## Where things live

| You want to... | Look at |
| --- | --- |
| Decide whether something belongs in michi at all | `PRINCIPLES.md` — the inclusion checklist |
| Add or change a rendering primitive | `src/<module>/` — see the module table in `README.md` |
| Touch anything behind a feature flag | `ARCHITECTURE.md` for the feature graph first |
| Change the NAPI/npm surface | `src/napi.rs` (the exports) + `packages/michi-node` (the cdylib shim) — not the other way around |
| Update the TOON grammar or wire format | `docs/spec/02-toon-format.md` is the source of truth; keep it and `src/toon/` in sync |
| Understand _why_ something is shaped the way it is | `docs/superpowers/specs/2026-07-03-michi-design.md` |

## Commit messages

Short, imperative, prefixed by kind — `fix:`, `feat:`, `chore:`, `test:`, `style:`, `ci:`. Look at `git log --oneline` for the house style before your first commit.

## Reporting bugs

Open a GitHub issue. If it's security-sensitive (something exploitable at the NAPI boundary, a dependency advisory, etc.), say so in the title and we'll prioritize accordingly — this is a young project without a formal disclosure process yet.
