/// Circuit breaker state machine (requires the `pipeline` feature).
#[cfg(feature = "pipeline")]
pub mod circuit;
/// Retry and back-off policy execution (requires the `pipeline` feature).
#[cfg(feature = "pipeline")]
pub mod policy;

use std::time::Duration;

/// Configuration for automatic retry behaviour.
///
/// Callers implement the retry loop; michi provides the delay computation via
/// [`next_retry_delay`]. This keeps the crate sync and runtime-agnostic.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (not counting the initial attempt).
    pub max_retries: u32,
    /// Base delay for exponential back-off.
    pub base_delay: Duration,
    /// Maximum delay cap — back-off never exceeds this.
    pub max_delay: Duration,
    /// Jitter factor in `[0.0, 1.0]`. `0.0` = no jitter, `1.0` = full jitter.
    pub jitter_factor: f64,
}

// Manually implemented because the defaults are meaningful non-zero values
// that `#[derive(Default)]` cannot express (it would set all durations to zero).
impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            jitter_factor: 0.2,
        }
    }
}

/// Compute the delay before the next retry attempt.
///
/// Uses exponential back-off: `base_delay * 2^attempt`, with optional jitter
/// derived from `jitter_seed` (a value in `[0.0, 1.0]` supplied by the caller
/// — use a PRNG, not `rand` inside michi) added to the pre-cap delay. If
/// `retry_after` is `Some`, the returned delay is the larger of the computed
/// backoff and `retry_after`. Either way, the result is then capped at
/// `max_delay`, so the returned delay never exceeds `max_delay` regardless of
/// `jitter_factor` or a server-supplied `Retry-After`.
///
/// Returns `None` when `attempt >= config.max_retries`.
#[must_use]
// f64 arithmetic is used only to scale the jitter factor; jitter_seed and
// jitter_factor are documented as [0.0, 1.0], so the product is non-negative
// and the truncating cast back to u64 cannot lose sign or go negative.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
pub fn next_retry_delay(
    config: &RetryConfig,
    attempt: u32,
    jitter_seed: f64,
    retry_after: Option<Duration>,
) -> Option<Duration> {
    if attempt >= config.max_retries {
        return None;
    }
    let exp = 2u64.saturating_pow(attempt);
    let base_ms = u64::try_from(config.base_delay.as_millis()).unwrap_or(u64::MAX);
    let max_ms = u64::try_from(config.max_delay.as_millis()).unwrap_or(u64::MAX);
    let raw_ms = base_ms.saturating_mul(exp);
    let jitter_ms = (raw_ms as f64 * config.jitter_factor * jitter_seed) as u64;
    let jittered_ms = raw_ms.saturating_add(jitter_ms);
    let retry_after_ms = retry_after.map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX));
    let capped_ms = jittered_ms.max(retry_after_ms).min(max_ms);
    Some(Duration::from_millis(capped_ms))
}

/// Parse the value of an HTTP `Retry-After` header as a delay in seconds.
///
/// Handles delay-seconds (integer) format. HTTP-date format is not parsed
/// (would require a date library dep) — returns `None` for dates.
/// Callers that need date parsing convert to seconds themselves.
#[must_use]
pub fn parse_retry_after(header_value: &str) -> Option<Duration> {
    header_value.trim().parse::<u64>().ok().map(Duration::from_secs)
}

/// Return `true` if the HTTP status code is conventionally retryable.
///
/// Retryable status codes: 429 (rate limit), 502, 503, 504 (gateway/upstream
/// unavailability). HTTP 500 is deliberately excluded — it signals a
/// server-side bug, and retrying an unchanged request reproduces the same
/// bug; retrying a write that returned 500 can also duplicate side effects if
/// the server processed the request before erroring. Callers that know a
/// specific API uses 500 for genuinely transient conditions can add it to
/// their own retry predicate independently of this function.
#[must_use]
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_retry_uses_base_delay() {
        let config = RetryConfig { jitter_factor: 0.0, ..Default::default() };
        let delay = next_retry_delay(&config, 0, 0.0, None).unwrap();
        assert_eq!(delay, config.base_delay);
    }

    #[test]
    fn second_retry_doubles() {
        let config = RetryConfig { jitter_factor: 0.0, ..Default::default() };
        let delay = next_retry_delay(&config, 1, 0.0, None).unwrap();
        assert_eq!(delay, config.base_delay * 2);
    }

    #[test]
    fn delay_is_capped_at_max() {
        let config = RetryConfig {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            jitter_factor: 0.0,
            max_retries: 10,
        };
        let delay = next_retry_delay(&config, 5, 0.0, None).unwrap();
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn beyond_max_retries_returns_none() {
        let config = RetryConfig::default();
        assert!(next_retry_delay(&config, 3, 0.0, None).is_none());
    }

    #[test]
    fn parse_retry_after_seconds() {
        assert_eq!(parse_retry_after("60"), Some(Duration::from_secs(60)));
        assert_eq!(parse_retry_after("  120  "), Some(Duration::from_secs(120)));
    }

    #[test]
    fn parse_retry_after_date_returns_none() {
        assert!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT").is_none());
    }

    #[test]
    fn retryable_status_codes() {
        assert!(is_retryable_status(429));
        assert!(is_retryable_status(503));
        assert!(!is_retryable_status(404));
        assert!(!is_retryable_status(400));
    }

    #[test]
    fn non_retryable_status_codes() {
        assert!(!is_retryable_status(200));
        assert!(!is_retryable_status(201));
        assert!(!is_retryable_status(301));
        assert!(!is_retryable_status(401));
    }

    #[test]
    fn http_500_is_not_retryable() {
        assert!(
            !is_retryable_status(500),
            "500 is a server bug — retrying reproduces it and risks duplicate side effects on writes"
        );
    }

    #[test]
    fn jitter_increases_delay() {
        let config = RetryConfig {
            jitter_factor: 1.0,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            max_retries: 3,
        };
        let no_jitter = next_retry_delay(&RetryConfig { jitter_factor: 0.0, ..config.clone() }, 0, 0.0, None).unwrap();
        let with_jitter = next_retry_delay(&config, 0, 1.0, None).unwrap();
        assert!(with_jitter > no_jitter, "jitter seed 1.0 should produce longer delay");
    }

    #[test]
    fn jitter_never_exceeds_max_delay_when_base_already_capped() {
        let config = RetryConfig {
            base_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(5),
            jitter_factor: 1.0,
            max_retries: 10,
        };
        // attempt 5 -> raw exponential delay is 32s, already well past the 5s cap.
        let delay = next_retry_delay(&config, 5, 1.0, None).unwrap();
        assert!(delay <= config.max_delay, "delay {delay:?} exceeded max_delay {:?}", config.max_delay);
    }

    #[test]
    fn extreme_duration_saturates_instead_of_wrapping() {
        // ~584 million years in seconds; as_millis() overflows u64, so a bare
        // `as u64` cast would silently wrap. This must saturate instead.
        let huge = Duration::from_secs(18_446_744_073_709_552);
        let config = RetryConfig { base_delay: huge, max_delay: huge, jitter_factor: 0.0, max_retries: 10 };
        let delay = next_retry_delay(&config, 0, 0.0, None).unwrap();
        assert!(delay.as_millis() > 1_000_000_000_000, "delay {delay:?} wrapped to a tiny value");
    }

    #[test]
    fn retry_after_wins_when_larger_than_backoff() {
        let config = RetryConfig { jitter_factor: 0.0, ..Default::default() };
        // attempt 0 backoff is base_delay (500ms). A 5s Retry-After should win.
        let delay = next_retry_delay(&config, 0, 0.0, Some(Duration::from_secs(5))).unwrap();
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn backoff_wins_when_larger_than_retry_after() {
        let config = RetryConfig { jitter_factor: 0.0, base_delay: Duration::from_secs(10), ..Default::default() };
        let delay = next_retry_delay(&config, 0, 0.0, Some(Duration::from_secs(1))).unwrap();
        assert_eq!(delay, Duration::from_secs(10));
    }

    #[test]
    fn retry_after_still_capped_at_max_delay() {
        let config = RetryConfig { jitter_factor: 0.0, max_delay: Duration::from_secs(5), ..Default::default() };
        let delay = next_retry_delay(&config, 0, 0.0, Some(Duration::from_secs(999))).unwrap();
        assert_eq!(delay, Duration::from_secs(5), "retry_after must not bypass max_delay");
    }

    #[test]
    fn none_retry_after_behaves_as_before() {
        let config = RetryConfig { jitter_factor: 0.0, ..Default::default() };
        let delay = next_retry_delay(&config, 0, 0.0, None).unwrap();
        assert_eq!(delay, config.base_delay);
    }
}
