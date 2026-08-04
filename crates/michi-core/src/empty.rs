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
}
