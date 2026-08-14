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
            ExecutionError::Failed { message, retryable } => {
                michi_core::DomainError::new(michi_core::ErrorCode::ExternalFailure, message).retryable(retryable)
            }
            ExecutionError::CircuitOpen { retry_after_ms } => michi_core::DomainError::new(
                michi_core::ErrorCode::Unavailable,
                format!("circuit open, retry after {retry_after_ms}ms"),
            )
            .retryable(true)
            .retry_after(std::time::Duration::from_millis(retry_after_ms)),
            ExecutionError::StepFailed { step_id, step_name, source } => {
                let inner: michi_core::DomainError = (*source).into();
                let message = format!("step {step_id} ({step_name}) failed: {}", inner.message);
                let mut domain_err = michi_core::DomainError::new(inner.code, message).retryable(inner.retryable);
                if let Some(retry_after) = inner.retry_after {
                    domain_err = domain_err.retry_after(retry_after);
                }
                domain_err
            }
            ExecutionError::StepCountMismatch { expected, got } => michi_core::DomainError::new(
                michi_core::ErrorCode::InvalidInput,
                format!("expected {expected} runners, got {got}"),
            )
            .retryable(false),
        }
    }
}

use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};

const PHASE_CLOSED: u8 = 0;
const PHASE_OPEN: u8 = 1;
const PHASE_HALF_OPEN: u8 = 2;

/// The circuit breaker's current phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerPhase {
    /// Calls are attempted normally.
    Closed,
    /// Calls are short-circuited without invoking the step.
    Open,
    /// A single probe call is allowed to test recovery.
    HalfOpen,
}

/// A circuit breaker composing michi-resilience's retry math with
/// tokio-clock-based async waiting, per-attempt timeout, and
/// consecutive-failure circuit-opening. Internal state is held in
/// lock-free atomics so [`CircuitBreaker::phase`] stays synchronous.
#[allow(dead_code)]
pub struct CircuitBreaker {
    retry_config: michi_resilience::RetryConfig,
    step_timeout: Duration,
    failure_threshold: u32,
    open_duration: Duration,
    phase: AtomicU8,
    opened_at_ms: AtomicU64,
    consecutive_failures: AtomicU32,
    epoch: tokio::time::Instant,
}

impl CircuitBreaker {
    /// Infallible, normalizing constructor. `failure_threshold == 0` is
    /// floored to 1; `step_timeout`/`open_duration` of `Duration::ZERO` are
    /// each independently floored to `Duration::from_millis(1)`.
    pub fn new(
        retry_config: michi_resilience::RetryConfig,
        step_timeout: Duration,
        failure_threshold: u32,
        open_duration: Duration,
    ) -> Self {
        let step_timeout = if step_timeout.is_zero() { Duration::from_millis(1) } else { step_timeout };
        let open_duration = if open_duration.is_zero() { Duration::from_millis(1) } else { open_duration };
        let failure_threshold = if failure_threshold == 0 { 1 } else { failure_threshold };
        Self {
            retry_config,
            step_timeout,
            failure_threshold,
            open_duration,
            phase: AtomicU8::new(PHASE_CLOSED),
            opened_at_ms: AtomicU64::new(0),
            consecutive_failures: AtomicU32::new(0),
            epoch: tokio::time::Instant::now(),
        }
    }

    /// Current phase. This early version only reflects the raw stored
    /// phase; the elapsed-time-based Open -> HalfOpen transition is added
    /// in a later commit once there is a real Open state to transition
    /// from.
    pub fn phase(&self) -> BreakerPhase {
        match self.phase.load(Ordering::SeqCst) {
            PHASE_OPEN => BreakerPhase::Open,
            PHASE_HALF_OPEN => BreakerPhase::HalfOpen,
            _ => BreakerPhase::Closed,
        }
    }
}

impl CircuitBreaker {
    /// Executes `step` to completion, retrying retryable failures per
    /// `retry_config` with `next_retry_delay`'s backoff, threading
    /// `ExecutionError::Http`'s `retry_after` through when present. Returns
    /// immediately on success or on a non-retryable error, and returns the
    /// last error once `next_retry_delay` reports exhaustion. Timeout and
    /// phase logic are added in later commits.
    pub async fn call(&self, step: &dyn Step, jitter_seed: f64) -> Result<(), ExecutionError> {
        let mut attempt = 0u32;
        loop {
            match step.run(attempt).await {
                Ok(()) => return Ok(()),
                Err(err) => {
                    if !err.is_retryable() {
                        return Err(err);
                    }
                    let retry_after = match &err {
                        ExecutionError::Http { retry_after, .. } => *retry_after,
                        _ => None,
                    };
                    match michi_resilience::next_retry_delay(&self.retry_config, attempt, jitter_seed, retry_after) {
                        Some(delay) => {
                            tokio::time::sleep(delay).await;
                            attempt += 1;
                        }
                        None => return Err(err),
                    }
                }
            }
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

        // Range boundaries: 502..=504 is inclusive on both ends, chosen over an
        // OR-pattern to satisfy clippy::manual_range_patterns -- assert both
        // edges so an accidental exclusive range (dropping 504) would be caught.
        let d: michi_core::DomainError =
            ExecutionError::Http { status: 502, message: "bad gateway".into(), retry_after: None }.into();
        assert_eq!(d.code, michi_core::ErrorCode::Unavailable);
        let d: michi_core::DomainError =
            ExecutionError::Http { status: 504, message: "gateway timeout".into(), retry_after: None }.into();
        assert_eq!(d.code, michi_core::ErrorCode::Unavailable);

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

    #[test]
    fn failed_conversion_preserves_retryable_flag() {
        let d: michi_core::DomainError = ExecutionError::Failed { message: "boom".into(), retryable: true }.into();
        assert_eq!(d.code, michi_core::ErrorCode::ExternalFailure);
        assert!(d.retryable);

        let d: michi_core::DomainError = ExecutionError::Failed { message: "boom".into(), retryable: false }.into();
        assert_eq!(d.code, michi_core::ErrorCode::ExternalFailure);
        assert!(!d.retryable);
    }

    #[test]
    fn step_failed_conversion_recurses_and_prefixes_message() {
        let source = ExecutionError::Failed { message: "disk full".into(), retryable: false };
        let err = ExecutionError::StepFailed {
            step_id: "upload".into(),
            step_name: "Upload".into(),
            source: Box::new(source),
        };
        let d: michi_core::DomainError = err.into();
        assert_eq!(d.code, michi_core::ErrorCode::ExternalFailure);
        assert!(!d.retryable);
        assert_eq!(d.message, "step upload (Upload) failed: disk full");
    }

    #[test]
    fn step_failed_conversion_preserves_inner_error_code() {
        // Uses a source variant whose code differs from ExternalFailure so this
        // actually exercises the recursive `inner.code` read, rather than being
        // indistinguishable from a hardcoded ExternalFailure (as Failed's own
        // ExternalFailure code would be) -- a mutation-testing checkpoint found
        // the existing StepFailed test alone couldn't catch a hardcode regression.
        let source = ExecutionError::Timeout { elapsed_ms: 5 };
        let err = ExecutionError::StepFailed { step_id: "s".into(), step_name: "S".into(), source: Box::new(source) };
        let d: michi_core::DomainError = err.into();
        assert_eq!(d.code, michi_core::ErrorCode::Timeout);
    }

    #[test]
    fn step_failed_conversion_inherits_inner_retry_after() {
        // Neither of the two tests above ever produces a Some(retry_after) on
        // the inner DomainError, so the `if let Some(retry_after) = ...`
        // branch in the StepFailed arm was previously untested -- a review
        // found this gap. CircuitOpen always sets retry_after, so it's used
        // here specifically to exercise that branch.
        let source = ExecutionError::CircuitOpen { retry_after_ms: 500 };
        let err = ExecutionError::StepFailed { step_id: "s".into(), step_name: "S".into(), source: Box::new(source) };
        let d: michi_core::DomainError = err.into();
        assert_eq!(d.retry_after, Some(std::time::Duration::from_millis(500)));
    }

    #[test]
    fn new_is_infallible_and_fresh_breaker_is_closed() {
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            std::time::Duration::ZERO,
            u32::MAX,
            std::time::Duration::ZERO,
        );
        assert_eq!(breaker.phase(), BreakerPhase::Closed);
    }

    struct CountingStep {
        calls: std::sync::atomic::AtomicU32,
    }
    impl Step for CountingStep {
        fn run<'a>(&'a self, _attempt: u32) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async { Ok(()) })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn call_succeeds_in_one_attempt_with_zero_virtual_time() {
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(1),
            u32::MAX,
            Duration::from_secs(60),
        );
        let step = CountingStep { calls: std::sync::atomic::AtomicU32::new(0) };
        let before = tokio::time::Instant::now();
        let result = breaker.call(&step, 0.0).await;
        let elapsed = before.elapsed();
        assert!(result.is_ok());
        assert_eq!(step.calls.load(Ordering::SeqCst), 1);
        assert_eq!(elapsed, Duration::ZERO);
    }

    struct FlakyStep {
        fail_until: u32,
        calls: std::sync::atomic::AtomicU32,
    }
    impl Step for FlakyStep {
        fn run<'a>(&'a self, attempt: u32) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                if attempt < self.fail_until {
                    Err(ExecutionError::Failed { message: "flaky".into(), retryable: true })
                } else {
                    Ok(())
                }
            })
        }
    }

    struct AlwaysFailStep {
        retryable: bool,
        calls: std::sync::atomic::AtomicU32,
    }
    impl Step for AlwaysFailStep {
        fn run<'a>(&'a self, _attempt: u32) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let retryable = self.retryable;
            Box::pin(async move { Err(ExecutionError::Failed { message: "always".into(), retryable }) })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn retries_until_success_and_sums_delay() {
        let retry_config =
            michi_resilience::RetryConfig::new(5, Duration::from_millis(10), Duration::from_secs(1), 0.0);
        let breaker =
            CircuitBreaker::new(retry_config.clone(), Duration::from_secs(5), u32::MAX, Duration::from_secs(60));
        let step = FlakyStep { fail_until: 2, calls: std::sync::atomic::AtomicU32::new(0) };
        let before = tokio::time::Instant::now();
        let result = breaker.call(&step, 0.0).await;
        let elapsed = before.elapsed();
        assert!(result.is_ok());
        assert_eq!(step.calls.load(Ordering::SeqCst), 3);
        let expected: Duration =
            (0..2).filter_map(|a| michi_resilience::next_retry_delay(&retry_config, a, 0.0, None)).sum();
        assert_eq!(elapsed, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn retries_thread_http_retry_after() {
        let retry_config =
            michi_resilience::RetryConfig::new(5, Duration::from_millis(10), Duration::from_secs(1), 0.0);
        let breaker =
            CircuitBreaker::new(retry_config.clone(), Duration::from_secs(5), u32::MAX, Duration::from_secs(60));
        let ra = Duration::from_millis(300);
        struct HttpThenOk(std::sync::atomic::AtomicU32);
        impl Step for HttpThenOk {
            fn run<'a>(
                &'a self,
                attempt: u32,
            ) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Box::pin(async move {
                    if attempt == 0 {
                        Err(ExecutionError::Http {
                            status: 503,
                            message: "x".into(),
                            retry_after: Some(Duration::from_millis(300)),
                        })
                    } else {
                        Ok(())
                    }
                })
            }
        }
        let step = HttpThenOk(std::sync::atomic::AtomicU32::new(0));
        let before = tokio::time::Instant::now();
        let result = breaker.call(&step, 0.0).await;
        let elapsed = before.elapsed();
        assert!(result.is_ok());
        let expected = michi_resilience::next_retry_delay(&retry_config, 0, 0.0, Some(ra)).unwrap();
        assert_eq!(elapsed, expected);
    }

    #[tokio::test(start_paused = true)]
    async fn exhausts_retries_then_fails() {
        let retry_config =
            michi_resilience::RetryConfig::new(3, Duration::from_millis(1), Duration::from_millis(10), 0.0);
        let breaker = CircuitBreaker::new(retry_config, Duration::from_secs(5), u32::MAX, Duration::from_secs(60));
        let step = AlwaysFailStep { retryable: true, calls: std::sync::atomic::AtomicU32::new(0) };
        let result = breaker.call(&step, 0.0).await;
        assert!(result.is_err());
        assert_eq!(step.calls.load(Ordering::SeqCst), 4);
    }

    #[tokio::test(start_paused = true)]
    async fn non_retryable_failure_returns_after_one_attempt_zero_time() {
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            u32::MAX,
            Duration::from_secs(60),
        );
        let step = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        let before = tokio::time::Instant::now();
        let result = breaker.call(&step, 0.0).await;
        let elapsed = before.elapsed();
        assert!(result.is_err());
        assert_eq!(step.calls.load(Ordering::SeqCst), 1);
        assert_eq!(elapsed, Duration::ZERO);
    }
}
