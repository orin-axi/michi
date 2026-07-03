use crate::hints::Hint;

/// Render a definitive empty state response.
///
/// Produces a TOON-compatible empty block: `type_name[0]{}:\ntotalCount: 0\n`.
/// Agents interpret this as "the collection exists but is genuinely empty" —
/// distinct from an error or a missing resource.
#[must_use]
pub fn empty_state(type_name: &str) -> String {
    let mut out = String::with_capacity(type_name.len() + 20);
    out.push_str(type_name);
    out.push_str("[0]{}:\ntotalCount: 0\n");
    out
}

/// Render a definitive empty state with contextual usage hints.
///
/// Equivalent to `empty_state(type_name)` followed by appending a `help[N]:`
/// block.
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
    fn empty_state_with_hints_appends_help_block() {
        let hints = [Hint::new("Try a broader filter")];
        let out = empty_state_with_hints("issue", &hints);
        assert_eq!(
            out,
            "issue[0]{}:\ntotalCount: 0\nhelp[1]:\n  Try a broader filter\n"
        );
    }

    #[test]
    fn empty_state_with_no_hints_matches_plain() {
        assert_eq!(empty_state_with_hints("task", &[]), empty_state("task"));
    }

    #[test]
    fn type_name_is_used_verbatim() {
        assert!(empty_state("my_resource").starts_with("my_resource[0]"));
    }
}
