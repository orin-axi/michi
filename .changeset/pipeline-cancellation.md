---
michi-pipeline: major
---

`execute_pipeline()` and `execute_pipeline_parallel()` each take a new trailing `cancellation: &CancellationToken` parameter, letting a caller cooperatively cancel an in-flight or not-yet-started run. Steps that haven't started when cancellation is observed are marked `Skipped`; an already-running step completes to its own real terminal status rather than being interrupted. A cancelled run returns `Err(ExecutionError::Cancelled)` (a new, `#[non_exhaustive]`-safe variant on `ExecutionError`).

Existing callers must pass a `CancellationToken` at every call site — construct `CancellationToken::new()` and never call `.cancel()` on it to preserve today's unconditional-run behavior.
