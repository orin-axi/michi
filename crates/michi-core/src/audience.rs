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

    #[test]
    fn ac001_is_non_exhaustive() {
        let src = include_str!("audience.rs");
        assert!(src.contains("#[non_exhaustive]"), "missing #[non_exhaustive]");
    }

    #[test]
    fn ac001_is_eq() {
        fn assert_eq_impl<T: Eq>() {}
        assert_eq_impl::<Audience>();
    }

    #[test]
    #[cfg(feature = "serde")]
    fn ac037_serializes_lowercase() {
        assert_eq!(serde_json::to_string(&Audience::Assistant).unwrap(), "\"assistant\"");
        assert_eq!(serde_json::to_string(&Audience::User).unwrap(), "\"user\"");
    }
}
