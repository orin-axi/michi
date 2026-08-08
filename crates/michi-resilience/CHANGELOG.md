# michi-resilience

## 0.1.1

### Patch Changes

- `nextRetryDelay` now rejects `NaN` and `Infinity` for `jitterFactor` and `jitterSeed` with a clear error. Previously, these values silently zeroed jitter and caused synchronized retry storms. `RetryConfig::new` is hardened the same way for Rust callers.

