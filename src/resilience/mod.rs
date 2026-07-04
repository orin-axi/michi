#[cfg(feature = "pipeline")]
pub mod circuit;
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
/// Uses exponential back-off: `base_delay * 2^attempt`, capped at
/// `max_delay`, with optional jitter derived from `jitter_seed` (a value in
/// `[0.0, 1.0]` supplied by the caller — use a PRNG, not `rand` inside michi).
///
/// Returns `None` when `attempt >= config.max_retries`.
#[must_use]
// Casts are safe: delays are capped at max_delay (≤ 30 s by default, well within u64 ms range).
// jitter_seed is documented as [0.0, 1.0], so the f64 product is non-negative and bounded.
#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss, clippy::cast_sign_loss)]
pub fn next_retry_delay(config: &RetryConfig, attempt: u32, jitter_seed: f64) -> Option<Duration> {
    if attempt >= config.max_retries {
        return None;
    }
    let exp = 2u64.saturating_pow(attempt);
    let base_ms = config.base_delay.as_millis() as u64;
    let raw_ms = base_ms.saturating_mul(exp);
    let capped_ms = raw_ms.min(config.max_delay.as_millis() as u64);
    let jitter_ms = (capped_ms as f64 * config.jitter_factor * jitter_seed) as u64;
    Some(Duration::from_millis(capped_ms + jitter_ms))
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
/// Retryable status codes: 429 (rate limit), 500, 502, 503, 504.
#[must_use]
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 500 | 502 | 503 | 504)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_retry_uses_base_delay() {
        let config = RetryConfig { jitter_factor: 0.0, ..Default::default() };
        let delay = next_retry_delay(&config, 0, 0.0).unwrap();
        assert_eq!(delay, config.base_delay);
    }

    #[test]
    fn second_retry_doubles() {
        let config = RetryConfig { jitter_factor: 0.0, ..Default::default() };
        let delay = next_retry_delay(&config, 1, 0.0).unwrap();
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
        let delay = next_retry_delay(&config, 5, 0.0).unwrap();
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn beyond_max_retries_returns_none() {
        let config = RetryConfig::default();
        assert!(next_retry_delay(&config, 3, 0.0).is_none());
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
    fn jitter_increases_delay() {
        let config = RetryConfig {
            jitter_factor: 1.0,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(30),
            max_retries: 3,
        };
        let no_jitter = next_retry_delay(&RetryConfig { jitter_factor: 0.0, ..config.clone() }, 0, 0.0).unwrap();
        let with_jitter = next_retry_delay(&config, 0, 1.0).unwrap();
        assert!(with_jitter > no_jitter, "jitter seed 1.0 should produce longer delay");
    }
}
