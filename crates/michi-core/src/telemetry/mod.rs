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

    #[test]
    fn ac021_default_and_unit_literal_both_construct_a_copyable_provider() {
        let a = NoopProvider::default();
        let b = NoopProvider;
        let c = a;
        // `a` must still be usable after the copy — proves Copy, not just Clone.
        let _ = (a, b, c);
    }

    #[test]
    fn ac022_span_accepts_the_specified_args_and_returns_unit() {
        let p = NoopProvider;
        p.span("");
        p.span("span-1");
        p.span("スパン");
        let x: () = p.span("span-1");
        let _ = x;
    }

    #[test]
    fn ac023_count_accepts_every_specified_name_value_combination() {
        let p = NoopProvider;
        for name in ["", "counter"] {
            for value in [0u64, 1, u64::MAX] {
                p.count(name, value);
            }
        }
    }

    #[test]
    fn ac024_1000_interleaved_calls_do_not_panic_and_provider_is_zero_sized() {
        let p = NoopProvider;
        for _ in 0..1000 {
            p.span("s");
            p.count("c", 1);
        }
        p.span("s");
        p.count("c", u64::MAX);
        assert_eq!(std::mem::size_of::<NoopProvider>(), 0);
    }

    // AC-028 (NoopProvider half — the StepStatus/PipelineStep/Pipeline half
    // lives in pipeline::tests::ac028_debug_output_is_exact).
    #[test]
    fn ac028_debug_output_is_exact() {
        assert_eq!(format!("{NoopProvider:?}"), "NoopProvider");
    }
}
