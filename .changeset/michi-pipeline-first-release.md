---
michi-pipeline: none
---

`michi-pipeline` is a new crate (this is its first release) that executes `michi-core`'s `Pipeline`/`PipelineStep`/`StepStatus` data model. It provides:

- `Step`, an object-safe async trait (`Vec<Box<dyn Step>>` usable without an async-trait dependency), plus a `step_fn` adapter for plain async closures.
- `CircuitBreaker`, composing `michi-resilience`'s retry math with tokio-clock-based waiting: per-attempt timeout, retry with backoff/jitter on retryable failures, and consecutive-failure circuit-opening with `Closed`/`Open`/`HalfOpen` phases (`HalfOpen` allows exactly one single-flight probe call).
- `execute_pipeline`, running a pipeline's steps sequentially through a `CircuitBreaker` and writing each step's real outcome back into `PipelineStep.status`.
- `execute_pipeline_parallel`, running steps concurrently according to a caller-declared `StepDependencies` graph — a step starts only once every step it depends on has reached a terminal status; on any step failure, every transitive dependent is written `Skipped` while unrelated in-flight steps still run to their own real terminal status.
- `CancellationToken`, a cheap-to-clone cooperative cancellation signal accepted by both `execute_pipeline` and `execute_pipeline_parallel`: steps that haven't started when cancellation is observed are marked `Skipped`, an already-running step still completes to its own real terminal status, and a cancelled run returns `Err(ExecutionError::Cancelled)`.
- `ExecutionError`, the crate's own `#[non_exhaustive]` error type (not a `michi_core::Error` variant), with `is_retryable()` classification and a `From<ExecutionError> for michi_core::DomainError` conversion for rendering failures through `michi-core`'s response primitives.

`Cargo.toml` already lists this crate at `0.1.0`, so this changeset documents the release without bumping the version.
