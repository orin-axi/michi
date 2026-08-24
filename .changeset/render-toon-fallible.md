---
michi-toon: minor
michi: minor
---

`render_toon()` now returns `Result<String, ToonError>` instead of `String`. Previously, invalid input (row/field arity mismatches, oversized rows, structural characters in names) degraded silently to a best-effort or malformed string; now every validation failure is surfaced as an explicit `Err`. The loose `render()` function (the pre-`ToonDocument` code path with debug/release-divergent behavior) has been removed — `render_toon()` and `list()` are the only entry points, both routed through `ToonDocument`'s validated construction path.

Callers must add `?` or explicit error handling at `render_toon()` call sites.

`ToonDocument` — the proof-carrying type this validated construction path routes through (`ToonDocument::validate(&opts)` fallibly constructs, `.render()` infallibly renders) — is now re-exported at the `michi-toon` crate root, so callers who want the validate/render split explicitly no longer need to go through `render_toon()`.
