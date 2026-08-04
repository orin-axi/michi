#![deny(unsafe_code)]
#![warn(missing_docs)]

//! # michi-resilience
//!
//! Retry configuration, delay calculation, RFC 7231 `Retry-After` parsing,
//! and FNV-1a idempotency primitives. Zero runtime dependencies.

use std::time::Duration;

/// Configuration for automatic retry behaviour.
#[derive(Debug, Clone)]
#[non_exhaustive]
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

impl RetryConfig {
    /// Create a new RetryConfig with custom values.
    #[must_use]
    pub fn new(max_retries: u32, base_delay: Duration, max_delay: Duration, jitter_factor: f64) -> Self {
        Self { max_retries, base_delay, max_delay, jitter_factor }
    }
}

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
#[must_use]
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
#[must_use]
pub fn parse_retry_after(header_value: &str) -> Option<Duration> {
    parse_retry_after_at(header_value, std::time::SystemTime::now())
}

/// Like [`parse_retry_after`], but takes the current time explicitly.
#[must_use]
pub fn parse_retry_after_at(header_value: &str, now: std::time::SystemTime) -> Option<Duration> {
    let trimmed = header_value.trim();
    if let Ok(secs) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(secs));
    }
    let target = parse_http_date(trimmed)?;
    Some(target.duration_since(now).unwrap_or(Duration::ZERO))
}

fn parse_http_date(s: &str) -> Option<std::time::SystemTime> {
    let s = s.strip_suffix(" GMT")?;
    let (_weekday, rest) = s.split_once(", ")?;
    let mut parts = rest.split(' ');
    let day: u64 = parts.next()?.parse().ok()?;
    let month = month_number(parts.next()?)?;
    let year: i64 = parts.next()?.parse().ok()?;
    if !(0..=9999).contains(&year) {
        return None;
    }
    let time = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let mut time_parts = time.split(':');
    let hour: u64 = time_parts.next()?.parse().ok()?;
    let minute: u64 = time_parts.next()?.parse().ok()?;
    let second: u64 = time_parts.next()?.parse().ok()?;
    if time_parts.next().is_some() || hour > 23 || minute > 59 || second > 60 || day == 0 || day > 31 {
        return None;
    }

    let days = days_from_civil(year, month, day);
    #[allow(clippy::cast_possible_wrap)]
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

#[allow(clippy::cast_possible_wrap, clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn days_from_civil(y: i64, m: u64, d: u64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

/// Return `true` if the HTTP status code is conventionally retryable.
#[must_use]
pub fn is_retryable_status(status: u16) -> bool {
    matches!(status, 429 | 502 | 503 | 504)
}

/// An opaque idempotency key derived from operation inputs.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IdempotencyKey(pub String);

impl IdempotencyKey {
    /// Create an idempotency key from any string-like value.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// The raw key string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Construct a key from an operation name and raw input bytes, hashed with FNV-1a.
    #[must_use]
    pub fn from_hash(operation: &str, data: &[u8]) -> Self {
        Self(format!("{operation}:{:016x}", fnv1a_64(data)))
    }
}

impl From<String> for IdempotencyKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for IdempotencyKey {
    fn from(s: &str) -> Self {
        Self(s.into())
    }
}

/// Result of an idempotency check.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum AlreadyDone {
    /// The operation completed in a previous call.
    Yes {
        /// The previously stored result.
        result: String,
    },
    /// The operation has not been seen before — proceed with execution.
    No,
}

/// Check whether an operation has already completed.
#[must_use]
pub fn already_done(stored: Option<String>) -> AlreadyDone {
    match stored {
        Some(result) => AlreadyDone::Yes { result },
        None => AlreadyDone::No,
    }
}

/// Render an already-done response.
#[must_use]
pub fn render_already_done(operation: &str, summary: &str, hints: &[String]) -> String {
    let mut out = String::with_capacity(64 + operation.len() + summary.len() + hints.len() * 50);
    out.push_str("operation: ");
    out.push_str(operation);
    out.push_str("\nstatus:    already_done\nsummary:   ");
    out.push_str(summary);
    out.push('\n');
    if !hints.is_empty() {
        out.push_str("help[");
        out.push_str(&hints.len().to_string());
        out.push_str("]:\n");
        for hint in hints {
            out.push_str("  ");
            out.push_str(hint);
            out.push('\n');
        }
    }
    out
}

fn fnv1a_64(data: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_retry_uses_base_delay() {
        let config = RetryConfig::default();
        let delay = next_retry_delay(&config, 0, 0.0, None).expect("first retry delay calculated");
        assert_eq!(delay, config.base_delay);
    }

    #[test]
    fn parse_retry_after_seconds() {
        assert_eq!(parse_retry_after("60"), Some(Duration::from_secs(60)));
    }

    #[test]
    fn from_hash_is_deterministic() {
        let a = IdempotencyKey::from_hash("create_item", b"same input");
        let b = IdempotencyKey::from_hash("create_item", b"same input");
        assert_eq!(a, b);
    }

    fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn test_auto_traits() {
        assert_send_sync_static::<RetryConfig>();
        assert_send_sync_static::<IdempotencyKey>();
        assert_send_sync_static::<AlreadyDone>();
    }
}
