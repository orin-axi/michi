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

    /// Current phase. Open is derived dynamically: once at least
    /// `open_duration` of tokio virtual time has elapsed since the breaker
    /// most recently opened, this reports `HalfOpen` even though the stored
    /// atomic still holds `PHASE_OPEN`.
    pub fn phase(&self) -> BreakerPhase {
        let raw = self.phase.load(Ordering::SeqCst);
        if raw == PHASE_OPEN {
            let opened_at_ms = self.opened_at_ms.load(Ordering::SeqCst);
            let elapsed_ms = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
            let open_duration_ms = u64::try_from(self.open_duration.as_millis()).unwrap_or(u64::MAX);
            if elapsed_ms.saturating_sub(opened_at_ms) >= open_duration_ms {
                return BreakerPhase::HalfOpen;
            }
            return BreakerPhase::Open;
        }
        match raw {
            PHASE_HALF_OPEN => BreakerPhase::HalfOpen,
            _ => BreakerPhase::Closed,
        }
    }
}

impl CircuitBreaker {
    fn record_failure(&self) {
        let count = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
        if count >= self.failure_threshold {
            self.open_circuit();
        }
    }

    fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::SeqCst);
    }

    fn open_circuit(&self) {
        let elapsed_ms = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.opened_at_ms.store(elapsed_ms, Ordering::SeqCst);
        self.phase.store(PHASE_OPEN, Ordering::SeqCst);
    }
}

impl CircuitBreaker {
    /// Executes `step`, with behavior depending on [`CircuitBreaker::phase`].
    ///
    /// While `Open`, returns `Err(ExecutionError::CircuitOpen { retry_after_ms })`
    /// immediately without invoking `step` at all, where `retry_after_ms` is
    /// the time remaining until `open_duration` has elapsed since the
    /// breaker most recently opened.
    ///
    /// While `Closed`, retries retryable failures per `retry_config` with
    /// `next_retry_delay`'s backoff, threading `ExecutionError::Http`'s
    /// `retry_after` through when present. Returns immediately on success or
    /// on a non-retryable error, and returns the last error once
    /// `next_retry_delay` reports exhaustion. On success, resets the
    /// consecutive-failure counter; on a call-level failure (after retries
    /// are exhausted or the error is non-retryable), increments the counter
    /// once and opens the circuit at `failure_threshold`.
    ///
    /// While `HalfOpen`, runs `step` exactly once with no retries: success
    /// closes the circuit and resets the counter, and any failure reopens
    /// the circuit unconditionally, bypassing `failure_threshold`.
    pub async fn call(&self, step: &dyn Step, jitter_seed: f64) -> Result<(), ExecutionError> {
        let phase = self.phase();
        if phase == BreakerPhase::Open {
            let opened_at_ms = self.opened_at_ms.load(Ordering::SeqCst);
            let elapsed_ms = u64::try_from(self.epoch.elapsed().as_millis()).unwrap_or(u64::MAX);
            let open_duration_ms = u64::try_from(self.open_duration.as_millis()).unwrap_or(u64::MAX);
            let since_opened_ms = elapsed_ms.saturating_sub(opened_at_ms);
            let retry_after_ms = open_duration_ms.saturating_sub(since_opened_ms);
            return Err(ExecutionError::CircuitOpen { retry_after_ms });
        }
        if phase == BreakerPhase::HalfOpen {
            let attempt_result = match tokio::time::timeout(self.step_timeout, step.run(0)).await {
                Ok(result) => result,
                Err(_) => Err(ExecutionError::Timeout {
                    elapsed_ms: u64::try_from(self.step_timeout.as_millis()).unwrap_or(u64::MAX),
                }),
            };
            return match attempt_result {
                Ok(()) => {
                    self.phase.store(PHASE_CLOSED, Ordering::SeqCst);
                    self.record_success();
                    Ok(())
                }
                Err(err) => {
                    self.open_circuit();
                    Err(err)
                }
            };
        }
        let mut attempt = 0u32;
        loop {
            let attempt_result = match tokio::time::timeout(self.step_timeout, step.run(attempt)).await {
                Ok(result) => result,
                Err(_) => Err(ExecutionError::Timeout {
                    elapsed_ms: u64::try_from(self.step_timeout.as_millis()).unwrap_or(u64::MAX),
                }),
            };
            match attempt_result {
                Ok(()) => {
                    self.record_success();
                    return Ok(());
                }
                Err(err) => {
                    if !err.is_retryable() {
                        self.record_failure();
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
                        None => {
                            self.record_failure();
                            return Err(err);
                        }
                    }
                }
            }
        }
    }
}

/// Runs the steps of `pipeline` sequentially in declaration order through
/// `breaker`, writing each step's real outcome into `PipelineStep.status`.
pub async fn execute_pipeline(
    pipeline: &mut michi_core::pipeline::Pipeline,
    runners: Vec<Box<dyn Step>>,
    breaker: &CircuitBreaker,
    jitter_seed: f64,
) -> Result<(), ExecutionError> {
    if runners.len() != pipeline.steps.len() {
        return Err(ExecutionError::StepCountMismatch { expected: pipeline.steps.len(), got: runners.len() });
    }
    for (idx, runner) in runners.iter().enumerate() {
        let outcome = breaker.call(runner.as_ref(), jitter_seed).await;
        let step = &mut pipeline.steps[idx];
        match outcome {
            Ok(()) => {
                step.status = michi_core::pipeline::StepStatus::Completed;
            }
            Err(err) => {
                step.status = michi_core::pipeline::StepStatus::Failed;
                return Err(ExecutionError::StepFailed {
                    step_id: step.id.clone(),
                    step_name: step.name.clone(),
                    source: Box::new(err),
                });
            }
        }
    }
    Ok(())
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

    struct NeverCompletes;
    impl Step for NeverCompletes {
        fn run<'a>(&'a self, _attempt: u32) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
            Box::pin(async {
                tokio::time::sleep(Duration::from_secs(3600)).await;
                Ok(())
            })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn attempt_exceeding_step_timeout_fails_with_timeout_elapsed_ms() {
        let retry_config =
            michi_resilience::RetryConfig::try_new(0, Duration::from_millis(1), Duration::from_millis(1), 0.0).unwrap();
        let breaker = CircuitBreaker::new(retry_config, Duration::from_millis(500), u32::MAX, Duration::from_secs(60));
        let result = breaker.call(&NeverCompletes, 0.0).await;
        match result {
            Err(ExecutionError::Timeout { elapsed_ms }) => assert_eq!(elapsed_ms, 500),
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn zero_step_timeout_floors_to_one_millisecond() {
        let retry_config =
            michi_resilience::RetryConfig::try_new(0, Duration::from_millis(1), Duration::from_millis(1), 0.0).unwrap();
        let breaker = CircuitBreaker::new(retry_config, Duration::ZERO, u32::MAX, Duration::from_secs(60));
        let result = breaker.call(&NeverCompletes, 0.0).await;
        match result {
            Err(ExecutionError::Timeout { elapsed_ms }) => assert_eq!(elapsed_ms, 1),
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn jitter_seed_forwarded_unmodified_for_non_finite_values() {
        for seed in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 7.5, -3.0] {
            let retry_config =
                michi_resilience::RetryConfig::new(2, Duration::from_millis(10), Duration::from_secs(1), 0.5);
            let breaker =
                CircuitBreaker::new(retry_config.clone(), Duration::from_secs(5), u32::MAX, Duration::from_secs(60));
            let step = FlakyStep { fail_until: 1, calls: std::sync::atomic::AtomicU32::new(0) };
            let before = tokio::time::Instant::now();
            let result = breaker.call(&step, seed).await;
            let elapsed = before.elapsed();
            assert!(result.is_ok(), "seed {seed} should not panic or fail construction");
            let expected = michi_resilience::next_retry_delay(&retry_config, 0, seed, None).unwrap();
            assert_eq!(elapsed, expected, "seed {seed} elapsed mismatch");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn threshold_consecutive_failures_opens_circuit_once_per_call_not_per_attempt() {
        let retry_config =
            michi_resilience::RetryConfig::new(5, Duration::from_millis(1), Duration::from_millis(10), 0.0);
        let breaker = CircuitBreaker::new(retry_config, Duration::from_secs(5), 2, Duration::from_secs(60));
        let step = AlwaysFailStep { retryable: true, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&step, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Closed, "one failing call must not open a threshold-2 breaker");
        assert_eq!(
            step.calls.load(Ordering::SeqCst),
            6,
            "first call() must still exhaust all 6 attempts (max_retries=5)"
        );
        assert!(breaker.call(&step, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Open, "second failing call() must open the threshold-2 breaker");
    }

    #[tokio::test(start_paused = true)]
    async fn zero_failure_threshold_behaves_like_one() {
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            0,
            Duration::from_secs(60),
        );
        let step = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&step, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Open);
    }

    #[tokio::test(start_paused = true)]
    async fn success_resets_counter_so_four_non_consecutive_failures_never_open_a_threshold_three_breaker() {
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            3,
            Duration::from_secs(60),
        );
        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        let ok = CountingStep { calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert!(breaker.call(&ok, 0.0).await.is_ok());
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(
            breaker.phase(),
            BreakerPhase::Closed,
            "4 total failures but never 3 consecutive - must stay Closed"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn open_circuit_short_circuits_with_exact_retry_after_ms() {
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(10),
        );
        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Open);

        tokio::time::advance(Duration::from_secs(4)).await;

        let probe = CountingStep { calls: std::sync::atomic::AtomicU32::new(0) };
        let result = breaker.call(&probe, 0.0).await;
        assert_eq!(probe.calls.load(Ordering::SeqCst), 0, "step.run must not be invoked while Open");
        match result {
            Err(ExecutionError::CircuitOpen { retry_after_ms }) => assert_eq!(retry_after_ms, 6_000),
            other => panic!("expected CircuitOpen{{retry_after_ms: 6000}}, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn phase_transitions_to_half_open_once_open_duration_elapses() {
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(10),
        );
        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Open);

        let start = tokio::time::Instant::now();
        tokio::time::advance(Duration::from_secs(9)).await;
        assert_eq!(breaker.phase(), BreakerPhase::Open, "9s < 10s open_duration");
        tokio::time::advance(Duration::from_secs(1)).await;
        assert_eq!(breaker.phase(), BreakerPhase::HalfOpen, "10s >= 10s open_duration");
        assert_eq!(start.elapsed(), Duration::from_secs(10));
    }

    #[tokio::test(start_paused = true)]
    async fn zero_open_duration_floors_to_one_millisecond() {
        let breaker =
            CircuitBreaker::new(michi_resilience::RetryConfig::default(), Duration::from_secs(5), 1, Duration::ZERO);
        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Open, "0ms elapsed since opening, floored open_duration is 1ms");
        tokio::time::advance(Duration::from_millis(1)).await;
        assert_eq!(breaker.phase(), BreakerPhase::HalfOpen);
    }

    #[tokio::test(start_paused = true)]
    async fn half_open_success_closes_circuit_and_resets_counter() {
        // Uses failure_threshold=2, not 1: a review found the original
        // threshold=1 version made this test's reset-proof vacuous, since
        // count>=1 holds after a single post-recovery failure whether or not
        // the counter was actually reset (reset-then-+1=1 and
        // leftover-then-+1=2 both satisfy >=1). Verified by mutation testing:
        // deleting record_success() from the HalfOpen-success branch left the
        // old version's assertions unchanged. With threshold=2, a single
        // post-recovery failure only stays Closed if the counter was
        // genuinely reset to 0.
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            2,
            Duration::from_secs(10),
        );
        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Closed);
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Open);

        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(breaker.phase(), BreakerPhase::HalfOpen);

        let ok = CountingStep { calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&ok, 0.0).await.is_ok());
        assert_eq!(breaker.phase(), BreakerPhase::Closed);

        // Discriminating assertion: if the counter had NOT been reset, it
        // would already be at 2 from the failures above, and this one more
        // failure would push it to 3 >= 2, reopening the circuit.
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(
            breaker.phase(),
            BreakerPhase::Closed,
            "one post-recovery failure must not reopen a threshold-2 breaker if the counter was genuinely reset"
        );

        // A second post-recovery failure, from the correctly-reset baseline,
        // does reach the threshold -- confirming the counter still counts
        // correctly from 0, not stuck.
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(breaker.phase(), BreakerPhase::Open);
    }

    #[tokio::test(start_paused = true)]
    async fn half_open_failure_reopens_circuit_with_exactly_one_attempt_no_retries() {
        let retry_config =
            michi_resilience::RetryConfig::new(5, Duration::from_millis(1), Duration::from_millis(10), 0.0);
        let breaker = CircuitBreaker::new(retry_config, Duration::from_secs(5), 1, Duration::from_secs(10));
        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&fail, 0.0).await.is_err());
        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(breaker.phase(), BreakerPhase::HalfOpen);

        let step = FlakyStep { fail_until: 1, calls: std::sync::atomic::AtomicU32::new(0) };
        let result = breaker.call(&step, 0.0).await;
        assert!(result.is_err(), "HalfOpen probe must not retry into the eventual success");
        assert_eq!(step.calls.load(Ordering::SeqCst), 1, "exactly one attempt during HalfOpen");
        assert_eq!(breaker.phase(), BreakerPhase::Open);
    }

    struct PanicsStep;
    impl Step for PanicsStep {
        fn run<'a>(&'a self, _attempt: u32) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
            Box::pin(async { panic!("deliberate test panic") })
        }
    }

    #[tokio::test(start_paused = true)]
    async fn panicking_step_leaves_breaker_state_untouched() {
        let breaker = std::sync::Arc::new(CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            2,
            Duration::from_secs(60),
        ));
        let phase_before = breaker.phase();

        let handle = {
            let breaker = std::sync::Arc::clone(&breaker);
            tokio::spawn(async move { breaker.call(&PanicsStep, 0.0).await })
        };
        let join_result = handle.await;
        assert!(
            join_result.is_err(),
            "the panic must propagate out of call() as a JoinError, proving it is not caught internally"
        );
        assert_eq!(breaker.phase(), phase_before, "phase must be unchanged after the panic");

        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(
            breaker.call(&fail, 0.0).await.is_err(),
            "one real failing call, to prove the panic did not pre-contribute to the failure count"
        );
        assert_eq!(breaker.phase(), BreakerPhase::Closed, "panic must not have contributed a failure count");

        let ok = CountingStep { calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&ok, 0.0).await.is_ok(), "the breaker must remain usable after a panicking call");
        assert_eq!(breaker.phase(), BreakerPhase::Closed);
    }

    #[tokio::test(start_paused = true)]
    async fn half_open_failure_bypasses_failure_counter_unconditionally() {
        // A mutation-testing checkpoint found that swapping the HalfOpen-failure
        // branch's open_circuit() for record_failure() survives every black-box
        // test: by the time this branch is ever reached, consecutive_failures is
        // already >= failure_threshold (it can only get here via a prior Closed
        // -> Open transition), so record_failure()'s own threshold check would be
        // trivially true too -- the two calls are behaviorally indistinguishable
        // through call()/phase() alone. This white-box test locks in the actual
        // contract (unconditional reopen, no counter dependency) by reading the
        // private counter directly, documenting that open_circuit() is the
        // intended call, not an accident that happens to also satisfy the tests.
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(10),
        );
        let fail = AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) };
        assert!(breaker.call(&fail, 0.0).await.is_err());
        tokio::time::advance(Duration::from_secs(10)).await;
        assert_eq!(breaker.phase(), BreakerPhase::HalfOpen);

        let before = breaker.consecutive_failures.load(Ordering::SeqCst);
        assert!(breaker.call(&fail, 0.0).await.is_err());
        assert_eq!(
            breaker.consecutive_failures.load(Ordering::SeqCst),
            before,
            "HalfOpen failure must reopen unconditionally via open_circuit(), not via record_failure()'s threshold check"
        );
        assert_eq!(breaker.phase(), BreakerPhase::Open);
    }

    #[tokio::test(start_paused = true)]
    async fn retry_loop_terminates_within_bounded_attempts() {
        // A mutation-testing checkpoint found that a regression removing
        // `attempt += 1` from the retry loop doesn't fail cleanly -- it hangs
        // the test runner indefinitely under start_paused=true, since
        // next_retry_delay(attempt=0, ...) never reaches the max_retries
        // exhaustion check. Wrapping the call in an explicit outer timeout
        // converts that failure mode into a clean, fast assertion failure
        // instead of a hung CI job.
        let retry_config =
            michi_resilience::RetryConfig::new(3, Duration::from_millis(1), Duration::from_millis(10), 0.0);
        let breaker = CircuitBreaker::new(retry_config, Duration::from_secs(5), u32::MAX, Duration::from_secs(60));
        let step = AlwaysFailStep { retryable: true, calls: std::sync::atomic::AtomicU32::new(0) };
        let result = tokio::time::timeout(Duration::from_secs(5), breaker.call(&step, 0.0)).await;
        assert!(result.is_ok(), "retry loop must terminate (exhaust retries) well within a 5s bound, not hang");
        assert!(result.unwrap().is_err());
        assert_eq!(step.calls.load(Ordering::SeqCst), 4, "max_retries=3 -> 4 total attempts");
    }

    #[test]
    fn phase_reports_half_open_when_atomic_directly_holds_that_state() {
        // The `PHASE_HALF_OPEN => BreakerPhase::HalfOpen` arm in phase() is
        // never reached through this crate's own public API today -- HalfOpen
        // is always derived dynamically from PHASE_OPEN plus elapsed time
        // (T18), and nothing currently stores PHASE_HALF_OPEN directly. A
        // mutation-testing checkpoint confirmed deleting that arm survives
        // every other test for exactly this reason. It is kept as deliberate,
        // forward-compatible defensive code (documented at definition site,
        // T11) rather than dead code to delete, in case a future revision
        // stores PHASE_HALF_OPEN explicitly (e.g. for cross-call single-trial
        // enforcement). This white-box test exercises it directly via the
        // private atomic so the arm has real coverage rather than resting on
        // an argument about future-proofing alone.
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(10),
        );
        breaker.phase.store(PHASE_HALF_OPEN, Ordering::SeqCst);
        assert_eq!(breaker.phase(), BreakerPhase::HalfOpen);
    }

    fn make_pipeline(ids: &[&str]) -> michi_core::pipeline::Pipeline {
        michi_core::pipeline::Pipeline {
            id: "p".into(),
            steps: ids
                .iter()
                .map(|id| michi_core::pipeline::PipelineStep {
                    id: (*id).into(),
                    name: (*id).into(),
                    status: michi_core::pipeline::StepStatus::Pending,
                })
                .collect(),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn runner_count_greater_than_steps_returns_mismatch_without_mutating_statuses() {
        let mut pipeline = make_pipeline(&["a"]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(60),
        );
        let runners: Vec<Box<dyn Step>> = vec![
            Box::new(CountingStep { calls: std::sync::atomic::AtomicU32::new(0) }),
            Box::new(CountingStep { calls: std::sync::atomic::AtomicU32::new(0) }),
        ];
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        match result {
            Err(ExecutionError::StepCountMismatch { expected, got }) => {
                assert_eq!(expected, 1);
                assert_eq!(got, 2);
            }
            other => panic!("expected StepCountMismatch{{expected: 1, got: 2}}, got {other:?}"),
        }
        assert_eq!(pipeline.steps[0].status, michi_core::pipeline::StepStatus::Pending);
    }

    #[tokio::test(start_paused = true)]
    async fn runner_count_less_than_steps_returns_mismatch_without_mutating_statuses() {
        let mut pipeline = make_pipeline(&["a", "b"]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(60),
        );
        let runners: Vec<Box<dyn Step>> = vec![Box::new(CountingStep { calls: std::sync::atomic::AtomicU32::new(0) })];
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        match result {
            Err(ExecutionError::StepCountMismatch { expected, got }) => {
                assert_eq!(expected, 2);
                assert_eq!(got, 1);
            }
            other => panic!("expected StepCountMismatch{{expected: 2, got: 1}}, got {other:?}"),
        }
        for step in &pipeline.steps {
            assert_eq!(step.status, michi_core::pipeline::StepStatus::Pending);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn empty_pipeline_and_empty_runners_returns_ok() {
        let mut pipeline = make_pipeline(&[]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(60),
        );
        let runners: Vec<Box<dyn Step>> = Vec::new();
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        assert!(result.is_ok());
        assert!(pipeline.steps.is_empty());
    }

    fn split_toon_row(row: &str) -> Vec<String> {
        let mut fields = Vec::new();
        let mut chars = row.chars().peekable();
        while chars.peek().is_some() {
            let mut field = String::new();
            if chars.peek() == Some(&'"') {
                chars.next();
                while let Some(c) = chars.next() {
                    if c == '\\' {
                        if let Some(escaped) = chars.next() {
                            field.push(escaped);
                        }
                    } else if c == '"' {
                        break;
                    } else {
                        field.push(c);
                    }
                }
            } else {
                while let Some(&c) = chars.peek() {
                    if c == ',' {
                        break;
                    }
                    field.push(c);
                    chars.next();
                }
            }
            fields.push(field);
            if chars.peek() == Some(&',') {
                chars.next();
            }
        }
        fields
    }

    #[tokio::test(start_paused = true)]
    async fn all_success_pipeline_reports_completed_in_every_render_row() {
        let mut pipeline = make_pipeline(&["fetch", "upload", "notify"]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(60),
        );
        let runners: Vec<Box<dyn Step>> = (0..3)
            .map(|_| Box::new(CountingStep { calls: std::sync::atomic::AtomicU32::new(0) }) as Box<dyn Step>)
            .collect();
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        assert!(result.is_ok());
        for step in &pipeline.steps {
            assert_eq!(step.status, michi_core::pipeline::StepStatus::Completed);
        }

        let rendered = pipeline.render();
        let mut lines = rendered.lines();
        let _header = lines.next().unwrap();
        let mut row_count = 0;
        for line in lines {
            let row = line.strip_prefix("  ").unwrap_or(line);
            let fields = split_toon_row(row);
            assert_eq!(fields.len(), 3, "row {row:?} must have exactly 3 fields");
            assert_eq!(fields[2], "completed", "row {row:?} third field must be exactly 'completed'");
            row_count += 1;
        }
        assert_eq!(row_count, 3);
    }

    #[tokio::test(start_paused = true)]
    async fn render_field_exact_parsing_is_not_fooled_by_completed_in_step_name() {
        // The all-success fixture above never creates the condition AC-038's
        // field-exact parsing exists to guard: a row whose id/name literally
        // contains the substring "completed" but whose real status is NOT
        // completed. A naive `rendered.contains("completed")` check would be
        // fooled by this row; field-exact parsing (checking fields[2]
        // specifically) correctly reports it as failed.
        let mut pipeline = make_pipeline(&["get-completed-orders"]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(60),
        );
        let runners: Vec<Box<dyn Step>> =
            vec![Box::new(AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) })];
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        assert!(result.is_err());
        assert_eq!(pipeline.steps[0].status, michi_core::pipeline::StepStatus::Failed);

        let rendered = pipeline.render();
        assert!(
            rendered.contains("completed"),
            "sanity check: the row's name must literally contain the substring 'completed' for this test to exercise anything"
        );

        let mut lines = rendered.lines();
        let _header = lines.next().unwrap();
        let row = lines.next().unwrap();
        let row = row.strip_prefix("  ").unwrap_or(row);
        let fields = split_toon_row(row);
        assert_eq!(fields.len(), 3);
        assert_ne!(
            fields[2], "completed",
            "field-exact parsing must report this row's real status, not be fooled by 'completed' appearing in the name field"
        );
        assert_eq!(fields[2], "failed");
    }

    #[tokio::test(start_paused = true)]
    async fn pre_set_non_pending_status_does_not_skip_the_runner() {
        let mut pipeline = make_pipeline(&["a", "b"]);
        pipeline.steps[0].status = michi_core::pipeline::StepStatus::Completed;
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(60),
        );
        let runner0 = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        struct SpyStep(std::sync::Arc<std::sync::atomic::AtomicU32>);
        impl Step for SpyStep {
            fn run<'a>(
                &'a self,
                _attempt: u32,
            ) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok(()) })
            }
        }
        let runners: Vec<Box<dyn Step>> = vec![
            Box::new(SpyStep(std::sync::Arc::clone(&runner0))),
            Box::new(CountingStep { calls: std::sync::atomic::AtomicU32::new(0) }),
        ];
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        assert!(result.is_ok());
        assert_eq!(
            runner0.load(Ordering::SeqCst),
            1,
            "the pre-Completed step's runner must still be invoked exactly once"
        );
        assert_eq!(pipeline.steps[0].status, michi_core::pipeline::StepStatus::Completed);
    }

    #[tokio::test(start_paused = true)]
    async fn nth_step_failure_is_fail_fast_with_exact_source_and_statuses() {
        let mut pipeline = make_pipeline(&["a", "b", "c"]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            u32::MAX,
            Duration::from_secs(10),
        );
        let after_calls = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        struct SpyStep(std::sync::Arc<std::sync::atomic::AtomicU32>);
        impl Step for SpyStep {
            fn run<'a>(
                &'a self,
                _attempt: u32,
            ) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
                self.0.fetch_add(1, Ordering::SeqCst);
                Box::pin(async { Ok(()) })
            }
        }
        struct AlwaysFailsNonRetryableStep;
        impl Step for AlwaysFailsNonRetryableStep {
            fn run<'a>(
                &'a self,
                _attempt: u32,
            ) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
                Box::pin(async { Err(ExecutionError::Failed { message: "boom".into(), retryable: false }) })
            }
        }
        let runners: Vec<Box<dyn Step>> = vec![
            Box::new(CountingStep { calls: std::sync::atomic::AtomicU32::new(0) }),
            Box::new(AlwaysFailsNonRetryableStep),
            Box::new(SpyStep(std::sync::Arc::clone(&after_calls))),
        ];
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        assert_eq!(after_calls.load(Ordering::SeqCst), 0, "no runner after the failing step may be invoked");
        match result {
            Err(ExecutionError::StepFailed { step_id, step_name, source }) => {
                assert_eq!(step_id, "b");
                assert_eq!(step_name, "b");
                match *source {
                    ExecutionError::Failed { ref message, retryable } => {
                        assert_eq!(message, "boom");
                        assert!(!retryable);
                    }
                    other => panic!("expected the exact Failed source, got {other:?}"),
                }
            }
            other => panic!("expected StepFailed, got {other:?}"),
        }
        assert_eq!(pipeline.steps[0].status, michi_core::pipeline::StepStatus::Completed);
        assert_eq!(pipeline.steps[1].status, michi_core::pipeline::StepStatus::Failed);
        assert_eq!(pipeline.steps[2].status, michi_core::pipeline::StepStatus::Pending);
    }

    #[tokio::test(start_paused = true)]
    async fn execute_pipeline_never_writes_skipped() {
        let mut pipeline = make_pipeline(&["a", "b"]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(10),
        );
        let runners: Vec<Box<dyn Step>> = vec![
            Box::new(CountingStep { calls: std::sync::atomic::AtomicU32::new(0) }),
            Box::new(AlwaysFailStep { retryable: false, calls: std::sync::atomic::AtomicU32::new(0) }),
        ];
        let _ = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        for step in &pipeline.steps {
            assert_ne!(step.status, michi_core::pipeline::StepStatus::Skipped);
        }
    }

    #[tokio::test(start_paused = true)]
    async fn steps_execute_strictly_sequentially_with_a_sleep_window() {
        let mut pipeline = make_pipeline(&["s0", "s1", "s2"]);
        let breaker = CircuitBreaker::new(
            michi_resilience::RetryConfig::default(),
            Duration::from_secs(5),
            1,
            Duration::from_secs(60),
        );
        let log: std::sync::Arc<std::sync::Mutex<Vec<(&'static str, String)>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        struct MarkerStep {
            id: String,
            log: std::sync::Arc<std::sync::Mutex<Vec<(&'static str, String)>>>,
            sleep: Option<Duration>,
        }
        impl Step for MarkerStep {
            fn run<'a>(
                &'a self,
                _attempt: u32,
            ) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
                Box::pin(async move {
                    self.log.lock().unwrap_or_else(|e| e.into_inner()).push(("start", self.id.clone()));
                    if let Some(sleep) = self.sleep {
                        tokio::time::sleep(sleep).await;
                    }
                    self.log.lock().unwrap_or_else(|e| e.into_inner()).push(("end", self.id.clone()));
                    Ok(())
                })
            }
        }

        let runners: Vec<Box<dyn Step>> = vec![
            Box::new(MarkerStep {
                id: "s0".into(),
                log: std::sync::Arc::clone(&log),
                sleep: Some(Duration::from_millis(50)),
            }),
            Box::new(MarkerStep { id: "s1".into(), log: std::sync::Arc::clone(&log), sleep: None }),
            Box::new(MarkerStep { id: "s2".into(), log: std::sync::Arc::clone(&log), sleep: None }),
        ];
        let result = execute_pipeline(&mut pipeline, runners, &breaker, 0.0).await;
        assert!(result.is_ok());
        let recorded = log.lock().unwrap_or_else(|e| e.into_inner()).clone();
        assert_eq!(
            recorded,
            vec![
                ("start", "s0".to_string()),
                ("end", "s0".to_string()),
                ("start", "s1".to_string()),
                ("end", "s1".to_string()),
                ("start", "s2".to_string()),
                ("end", "s2".to_string()),
            ]
        );
    }

    #[tokio::test(start_paused = true)]
    async fn jitter_seed_is_identical_across_every_step() {
        let mut pipeline = make_pipeline(&["a", "b", "c"]);
        let retry_config =
            michi_resilience::RetryConfig::new(1, Duration::from_millis(10), Duration::from_millis(100), 0.5);
        let breaker = CircuitBreaker::new(retry_config.clone(), Duration::from_secs(5), 10, Duration::from_secs(60));

        struct SeedSpyStep;
        impl Step for SeedSpyStep {
            fn run<'a>(
                &'a self,
                attempt: u32,
            ) -> Pin<Box<dyn Future<Output = Result<(), ExecutionError>> + Send + 'a>> {
                Box::pin(async move {
                    if attempt == 0 {
                        Err(ExecutionError::Failed { message: "retry once".into(), retryable: true })
                    } else {
                        Ok(())
                    }
                })
            }
        }

        let runners: Vec<Box<dyn Step>> = (0..3).map(|_| Box::new(SeedSpyStep) as Box<dyn Step>).collect();

        let call_seed = 0.42_f64;
        let before = tokio::time::Instant::now();
        let result = execute_pipeline(&mut pipeline, runners, &breaker, call_seed).await;
        let elapsed = before.elapsed();
        assert!(result.is_ok());

        let per_step_delay = michi_resilience::next_retry_delay(&retry_config, 0, call_seed, None).unwrap();
        assert_eq!(
            elapsed,
            per_step_delay * 3,
            "all 3 steps must have used the same jitter_seed, producing 3 identical delays"
        );
    }
}
