use michi::resilience::{next_retry_delay, parse_retry_after, RetryConfig};
use proptest::prelude::*;
use std::time::Duration;

proptest! {
    #[test]
    fn parse_retry_after_never_panics(input in ".{0,200}") {
        let _ = parse_retry_after(&input);
    }

    #[test]
    fn next_retry_delay_within_max_delay_bound(
        attempt in 0u32..5,
        jitter_seed in 0.0f64..1.0,
        base_secs in 1u64..10,
        max_secs in 10u64..60,
        jitter_factor in 0.0f64..1.0,
    ) {
        let config = RetryConfig {
            max_retries: 10,
            base_delay: Duration::from_secs(base_secs),
            max_delay: Duration::from_secs(max_secs),
            jitter_factor,
        };
        if let Some(delay) = next_retry_delay(&config, attempt, jitter_seed, None) {
            prop_assert!(delay <= config.max_delay, "delay {delay:?} exceeded max_delay {:?}", config.max_delay);
        }
    }
}
