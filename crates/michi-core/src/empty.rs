use crate::hints::Hint;

/// Render a definitive empty state response.
#[must_use]
pub fn empty_state(type_name: &str) -> String {
    let mut out = String::with_capacity(type_name.len() + 20);
    out.push_str(type_name);
    out.push_str("[0]{}:\ntotalCount: 0\n");
    out
}

/// Render a definitive empty state with contextual usage hints.
#[must_use]
pub fn empty_state_with_hints(type_name: &str, hints: &[Hint]) -> String {
    let mut out = empty_state(type_name);
    crate::hints::append_hints(&mut out, hints);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_empty_state() {
        assert_eq!(empty_state("issue"), "issue[0]{}:\ntotalCount: 0\n");
    }

    #[test]
    fn ac003_empty_type_name_does_not_panic() {
        assert_eq!(empty_state(""), "[0]{}:\ntotalCount: 0\n");
    }

    #[test]
    fn ac004_capacity_is_at_least_type_name_len_plus_20() {
        let out = empty_state("issue");
        assert!(out.capacity() >= "issue".len() + 20, "capacity {} too small", out.capacity());
    }

    #[test]
    fn ac005_with_hints_appends_render_hints_output_in_place() {
        use crate::hints::Hint;
        let hints = [Hint::new("first"), Hint::new("second")];
        let out = empty_state_with_hints("issue", &hints);
        let base = empty_state("issue");
        assert!(out.starts_with(&base), "got: {out}");
        assert_eq!(&out[base.len()..], crate::hints::render_hints(&hints));
    }

    #[test]
    fn ac006_with_hints_and_empty_slice_equals_empty_state() {
        let out = empty_state_with_hints("issue", &[]);
        assert_eq!(out, empty_state("issue"));
    }
}
