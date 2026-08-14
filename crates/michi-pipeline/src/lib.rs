//! Executes michi-core's `Pipeline`/`PipelineStep`/`StepStatus` data model:
//! runs each step through a caller-supplied [`Step`] implementation, applies
//! retry/backoff and circuit-breaking on top of michi-resilience's pure
//! retry math, and writes each step's real outcome back into
//! `PipelineStep.status`.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]
