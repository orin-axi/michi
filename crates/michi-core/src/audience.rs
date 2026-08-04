//! Audience routing (`assistant` vs `user`).

/// Which surface a piece of content is meant for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[non_exhaustive]
pub enum Audience {
    /// The compact, token-efficient surface.
    Assistant,
    /// A human-readable surface.
    User,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audience_is_copy_and_comparable() {
        let a = Audience::Assistant;
        let b = a;
        assert_eq!(a, b);
        assert_ne!(Audience::Assistant, Audience::User);
    }
}
