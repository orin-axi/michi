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
    // Combine (max) before capping (min): retry_after must be able to raise the
    // delay above the jittered backoff, but max_delay must still be the final
    // word. Swapping this order would let retry_after bypass max_delay.
    let capped_ms = jittered_ms.max(retry_after_ms).min(max_ms);
    Some(Duration::from_millis(capped_ms))
}

/// Parse the value of an HTTP `Retry-After` header as a delay in seconds,
/// relative to the current wall-clock time.
///
/// Handles both forms from RFC 7231 §7.1.3: delay-seconds (`"120"`) and
/// HTTP-date (`"Wed, 21 Oct 2026 07:28:00 GMT"`, always UTC). Returns `None`
/// for malformed or absent values.
#[must_use]
pub fn parse_retry_after(header_value: &str) -> Option<Duration> {
    parse_retry_after_at(header_value, std::time::SystemTime::now())
}

/// Like [`parse_retry_after`], but takes the current time explicitly instead
/// of reading the system clock — deterministic and testable. `now` matters
/// only for the HTTP-date form; the delay-seconds form ignores it entirely.
#[must_use]
pub fn parse_retry_after_at(header_value: &str, now: std::time::SystemTime) -> Option<Duration> {
    let trimmed = header_value.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let target = parse_http_date(trimmed)?;
    Some(target.duration_since(now).unwrap_or(Duration::ZERO))
}

/// Parse a fixed-format RFC 7231 HTTP-date (`"Www, dd Mmm yyyy HH:MM:SS GMT"`)
/// into an absolute `SystemTime`. No timezone library needed — the format is
/// always GMT and fixed-width, so this is pure calendar arithmetic.
fn parse_http_date(s: &str) -> Option<std::time::SystemTime> {
    // "Wed, 21 Oct 2026 07:28:00 GMT"
    let s = s.strip_suffix(" GMT")?;
    let (_weekday, rest) = s.split_once(", ")?;
    let mut parts = rest.split(' ');
    let day: u64 = parts.next()?.parse().ok()?;
    let month = month_number(parts.next()?)?;
    let year: i64 = parts.next()?.parse().ok()?;
    let time = parts.next()?;
    if parts.next().is_some() {
        return None; // trailing garbage
    }
    let mut time_parts = time.split(':');
    let hour: u64 = time_parts.next()?.parse().ok()?;
    let minute: u64 = time_parts.next()?.parse().ok()?;
    let second: u64 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() || hour > 23 || minute > 59 || second > 60 || day == 0 || day > 31 {
        return None;
    }

    let days = days_from_civil(year, month, day);
    #[allow(
        clippy::cast_possible_wrap,
        reason = "hour/minute/second are bounded to <=23/<=59/<=60 above, so the sum fits comfortably in i64"
    )]
    let epoch_secs = days.checked_mul(86_400)?.checked_add((hour * 3600 + minute * 60 + second) as i64)?;
    let unix_secs = u64::try_from(epoch_secs).ok()?;
    Some(std::time::UNIX_EPOCH + Duration::from_secs(unix_secs))
}

fn month_number(name: &str) -> Option<u64> {
    Some(match name {
        "Jan" => 1,
        "Feb" => 2,
        "Mar" => 3,
        "Apr" => 4,
        "May" => 5,
        "Jun" => 6,
        "Jul" => 7,
        "Aug" => 8,
        "Sep" => 9,
        "Oct" => 10,
        "Nov" => 11,
        "Dec" => 12,
        _ => return None,
    })
}

/// Days since the Unix epoch for a given proleptic-Gregorian civil date.
/// Howard Hinnant's `days_from_civil` algorithm — no external date library,
/// correct for the full range this parser can produce (years > 1970).
#[allow(
    clippy::cast_possible_wrap,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "yoe and doy are bounded ([0,399] and [0,365] respectively) by the algorithm's own math, so \
              these casts cannot lose information for any year this parser accepts"
)]
fn days_from_civil(y: i64, m: u64, d: u64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64; // [0, 399]
    let mp = (m + 9) % 12; // [0, 11], Mar=0 .. Feb=11
    let doy = (153 * mp + 2) / 5 + d - 1; // [0, 365]
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy; // [0, 146096]
    era * 146_097 + doe as i64 - 719_468 // 719468 = days from 0000-03-01 to 1970-01-01
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
    fn parse_retry_after_date_in_past_clamps_to_zero() {
        // 2015 is in the past relative to the real clock `parse_retry_after` reads.
        assert_eq!(parse_retry_after("Wed, 21 Oct 2015 07:28:00 GMT"), Some(Duration::ZERO));
    }

    #[test]
    fn http_date_in_future_parses_relative_to_now() {
        // now = 2026-01-01T00:00:00Z, header = 2026-01-01T00:01:00Z -> 60s
        let now = std::time::UNIX_EPOCH + Duration::from_secs(1_767_225_600); // 2026-01-01T00:00:00Z
        let delay = parse_retry_after_at("Thu, 01 Jan 2026 00:01:00 GMT", now);
        assert_eq!(delay, Some(Duration::from_secs(60)));
    }

    #[test]
    fn http_date_in_past_clamps_to_zero() {
        let now = std::time::UNIX_EPOCH + Duration::from_secs(1_767_225_600); // 2026-01-01T00:00:00Z
        let delay = parse_retry_after_at("Wed, 31 Dec 2025 00:00:00 GMT", now);
        assert_eq!(delay, Some(Duration::ZERO), "past dates clamp to zero, per spec's clock-skew note");
    }

    #[test]
    fn malformed_http_date_returns_none() {
        let now = std::time::SystemTime::UNIX_EPOCH;
        assert_eq!(parse_retry_after_at("not a date", now), None);
        assert_eq!(parse_retry_after_at("Wed, 32 Foo 2026 00:00:00 GMT", now), None);
    }

    #[test]
    fn public_parse_retry_after_still_handles_seconds() {
        // regression guard: the public function must still handle the integer form
        assert_eq!(parse_retry_after("60"), Some(Duration::from_secs(60)));
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
    fn retry_after_wins_over_backoff_with_jitter_applied() {
        let config = RetryConfig { jitter_factor: 1.0, base_delay: Duration::from_secs(1), ..Default::default() };
        // jitter_seed 1.0 -> full jitter: jittered backoff is 1s + 1s = 2s,
        // still less than a 5s Retry-After.
        let delay = next_retry_delay(&config, 0, 1.0, Some(Duration::from_secs(5))).unwrap();
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn jittered_backoff_wins_over_smaller_retry_after() {
        let config = RetryConfig { jitter_factor: 1.0, base_delay: Duration::from_secs(1), ..Default::default() };
        // jitter_seed 1.0 -> full jitter: jittered backoff is 1s + 1s = 2s,
        // which exceeds the 500ms Retry-After.
        let delay = next_retry_delay(&config, 0, 1.0, Some(Duration::from_millis(500))).unwrap();
        assert_eq!(delay, Duration::from_secs(2));
    }
}
