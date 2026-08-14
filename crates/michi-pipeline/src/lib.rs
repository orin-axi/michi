//! Executes michi-core's `Pipeline`/`PipelineStep`/`StepStatus` data model:
//! runs each step through a caller-supplied [`Step`] implementation, applies
//! retry/backoff and circuit-breaking on top of michi-resilience's pure
//! retry math, and writes each step's real outcome back into
//! `PipelineStep.status`.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![warn(clippy::unwrap_used, clippy::expect_used)]

use std::time::Duration;

/// The crate's own execution-error type, implementing `std::error::Error`
/// via thiserror.
#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub enum ExecutionError {
    /// HTTP-transport failure.
    #[error("http {status}: {message}")]
    Http {
        /// HTTP status code returned by the failed request.
        status: u16,
        /// Human-readable failure message.
        message: String,
        /// Server-supplied retry delay, if the caller already parsed one.
        retry_after: Option<Duration>,
    },
    /// A step attempt exceeded its configured timeout.
    #[error("step timed out after {elapsed_ms}ms")]
    Timeout {
        /// Milliseconds elapsed before the attempt was aborted.
        elapsed_ms: u64,
    },
    /// Generic step failure not tied to HTTP transport.
    #[error("{message}")]
    Failed {
        /// Human-readable failure message.
        message: String,
        /// Whether this failure is worth retrying.
        retryable: bool,
    },
    /// The circuit breaker short-circuited the call without invoking the step.
    #[error("circuit open, retry after {retry_after_ms}ms")]
    CircuitOpen {
        /// Milliseconds remaining until the circuit may allow a probe call.
        retry_after_ms: u64,
    },
    /// A pipeline step failed, wrapping which step and its underlying cause.
    #[error("step {step_id} ({step_name}) failed: {source}")]
    StepFailed {
        /// The failed step's id.
        step_id: String,
        /// The failed step's name.
        step_name: String,
        /// The underlying error that caused this step to fail.
        source: Box<ExecutionError>,
    },
    /// `runners.len()` did not match `pipeline.steps.len()`.
    #[error("expected {expected} runners, got {got}")]
    StepCountMismatch {
        /// Expected runner count (`pipeline.steps.len()`).
        expected: usize,
        /// Actual runner count supplied (`runners.len()`).
        got: usize,
    },
}

impl ExecutionError {
    /// Classifies whether retrying is worthwhile for this error.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Http { status, .. } => michi_resilience::is_retryable_status(*status),
            Self::Timeout { .. } => true,
            Self::Failed { retryable, .. } => *retryable,
            Self::CircuitOpen { .. } => true,
            Self::StepFailed { source, .. } => source.is_retryable(),
            Self::StepCountMismatch { .. } => false,
        }
    }
}

use std::future::Future;
use std::pin::Pin;

/// What a pipeline step invokes. `attempt` is 0-indexed and supplied by
/// `CircuitBreaker::call`, incrementing by 1 on each retry. Object-safe by
/// construction, so `Vec<Box<dyn Step>>` is usable without an async-trait
/// dependency.
pub trait Step: Send + Sync {
    /// Runs one attempt of this step.
    fn run<'a>(&'a self, attempt: u32) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>>;
}

/// Adapter produced by [`step_fn`], implementing [`Step`] over an async
/// closure for callers who don't need custom state.
pub struct FnStep<F>(F);

/// Ergonomic adapter from an async closure into a value implementing
/// [`Step`], for callers who don't need custom state.
pub fn step_fn<F, Fut>(f: F) -> FnStep<F>
where
    F: Fn(u32) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), ExecutionError>> + Send + 'static,
{
    FnStep(f)
}

impl<F, Fut> Step for FnStep<F>
where
    F: Fn(u32) -> Fut + Send + Sync,
    Fut: Future<Output = Result<(), ExecutionError>> + Send + 'static,
{
    fn run<'a>(&'a self, attempt: u32) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
        Box::pin((self.0)(attempt))
    }
}

impl From<ExecutionError> for michi_core::DomainError {
    fn from(err: ExecutionError) -> Self {
        match err {
            ExecutionError::Http { status, message, retry_after } => {
                let code = match status {
                    429 => michi_core::ErrorCode::RateLimited,
                    502..=504 => michi_core::ErrorCode::Unavailable,
                    _ => michi_core::ErrorCode::ExternalFailure,
                };
                let retryable = michi_resilience::is_retryable_status(status);
                let mut domain_err = michi_core::DomainError::new(code, message).retryable(retryable);
                if let Some(retry_after) = retry_after {
                    domain_err = domain_err.retry_after(retry_after);
                }
                domain_err
            }
            ExecutionError::Timeout { elapsed_ms } => michi_core::DomainError::new(
                michi_core::ErrorCode::Timeout,
                format!("step timed out after {elapsed_ms}ms"),
            )
            .retryable(true),
            ExecutionError::Failed { message, .. } => {
                michi_core::DomainError::new(michi_core::ErrorCode::ExternalFailure, message)
            }
            ExecutionError::CircuitOpen { retry_after_ms } => michi_core::DomainError::new(
                michi_core::ErrorCode::Unavailable,
                format!("circuit open, retry after {retry_after_ms}ms"),
            )
            .retryable(true)
            .retry_after(std::time::Duration::from_millis(retry_after_ms)),
            ExecutionError::StepFailed { .. } => {
                michi_core::DomainError::new(michi_core::ErrorCode::ExternalFailure, "placeholder")
            }
            ExecutionError::StepCountMismatch { expected, got } => michi_core::DomainError::new(
                michi_core::ErrorCode::InvalidInput,
                format!("expected {expected} runners, got {got}"),
            )
            .retryable(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn execution_error_has_six_variants_and_impls_std_error() {
        fn assert_std_error<E: std::error::Error>() {}
        assert_std_error::<ExecutionError>();

        let _http = ExecutionError::Http { status: 500, message: "x".into(), retry_after: None };
        let _timeout = ExecutionError::Timeout { elapsed_ms: 1 };
        let _failed = ExecutionError::Failed { message: "x".into(), retryable: true };
        let _circuit_open = ExecutionError::CircuitOpen { retry_after_ms: 1 };
        let _step_failed = ExecutionError::StepFailed {
            step_id: "s".into(),
            step_name: "S".into(),
            source: Box::new(ExecutionError::Timeout { elapsed_ms: 1 }),
        };
        let _mismatch = ExecutionError::StepCountMismatch { expected: 1, got: 2 };
    }

    struct AlwaysOk;
    impl Step for AlwaysOk {
        fn run<'a>(
            &'a self,
            _attempt: u32,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
            Box::pin(async { Ok(()) })
        }
    }

    #[test]
    fn step_is_object_safe() {
        let steps: Vec<Box<dyn Step>> = vec![Box::new(AlwaysOk)];
        assert_eq!(steps.len(), 1);
    }

    #[tokio::test]
    async fn step_fn_adapts_closure_into_step() {
        let steps: Vec<Box<dyn Step>> = vec![Box::new(step_fn(|_attempt: u32| async { Ok(()) }))];
        let result = steps[0].run(0).await;
        assert!(result.is_ok());
    }

    #[test]
    fn is_retryable_per_variant() {
        assert_eq!(
            ExecutionError::Http { status: 429, message: "x".into(), retry_after: None }.is_retryable(),
            michi_resilience::is_retryable_status(429)
        );
        assert_eq!(
            ExecutionError::Http { status: 400, message: "x".into(), retry_after: None }.is_retryable(),
            michi_resilience::is_retryable_status(400)
        );
        assert!(ExecutionError::Timeout { elapsed_ms: 1 }.is_retryable());
        assert!(ExecutionError::Failed { message: "x".into(), retryable: true }.is_retryable());
        assert!(!ExecutionError::Failed { message: "x".into(), retryable: false }.is_retryable());
        assert!(ExecutionError::CircuitOpen { retry_after_ms: 1 }.is_retryable());
        assert!(ExecutionError::StepFailed {
            step_id: "s".into(),
            step_name: "S".into(),
            source: Box::new(ExecutionError::Timeout { elapsed_ms: 1 }),
        }
        .is_retryable());
        assert!(!ExecutionError::StepFailed {
            step_id: "s".into(),
            step_name: "S".into(),
            source: Box::new(ExecutionError::Failed { message: "x".into(), retryable: false }),
        }
        .is_retryable());
        assert!(!ExecutionError::StepCountMismatch { expected: 1, got: 2 }.is_retryable());
    }

    #[test]
    fn http_conversion_maps_status_groups() {
        let d: michi_core::DomainError = ExecutionError::Http {
            status: 429,
            message: "rate limited".into(),
            retry_after: Some(Duration::from_secs(2)),
        }
        .into();
        assert_eq!(d.code, michi_core::ErrorCode::RateLimited);
        assert!(d.retryable);
        assert_eq!(d.retry_after, Some(Duration::from_secs(2)));

        let d: michi_core::DomainError =
            ExecutionError::Http { status: 503, message: "unavailable".into(), retry_after: None }.into();
        assert_eq!(d.code, michi_core::ErrorCode::Unavailable);
        assert!(d.retryable);
        assert_eq!(d.retry_after, None);

        let d: michi_core::DomainError =
            ExecutionError::Http { status: 400, message: "bad request".into(), retry_after: None }.into();
        assert_eq!(d.code, michi_core::ErrorCode::ExternalFailure);
        assert!(!d.retryable);
    }

    #[test]
    fn timeout_circuit_open_and_mismatch_conversions() {
        let d: michi_core::DomainError = ExecutionError::Timeout { elapsed_ms: 250 }.into();
        assert_eq!(d.code, michi_core::ErrorCode::Timeout);
        assert!(d.retryable);

        let d: michi_core::DomainError = ExecutionError::CircuitOpen { retry_after_ms: 750 }.into();
        assert_eq!(d.code, michi_core::ErrorCode::Unavailable);
        assert!(d.retryable);
        assert_eq!(d.retry_after, Some(std::time::Duration::from_millis(750)));

        let d: michi_core::DomainError = ExecutionError::StepCountMismatch { expected: 3, got: 1 }.into();
        assert_eq!(d.code, michi_core::ErrorCode::InvalidInput);
        assert!(!d.retryable);
    }
}
