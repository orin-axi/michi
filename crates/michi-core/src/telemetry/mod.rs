/// No-op telemetry provider (zero-cost, always compiled).
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopProvider;

impl NoopProvider {
    /// Record a named span (no-op).
    #[inline]
    pub fn span(&self, _name: &str) {}

    /// Increment a named counter (no-op).
    #[inline]
    pub fn count(&self, _name: &str, _value: u64) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noop_provider_compiles() {
        let p = NoopProvider;
        p.span("test");
        p.count("test", 1);
    }
}
