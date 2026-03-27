use {
    core::{fmt, num::NonZero},
    wyrand::WyRand,
};

/// A non-`Clone` wrapper around `usize`
/// to prevent accounting errors.
#[derive(Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Size {
    /// The internal size value that must not be `Clone`d.
    size: usize,
}

impl Size {
    #[inline]
    pub fn expanding() -> impl Iterator<Item = Self> {
        (0..).map(|size| Self { size })
    }

    /// Whether to choose a potential leaf or loop constructor.
    #[must_use]
    #[inline]
    pub fn should_recurse(&self, prng: &mut WyRand) -> bool {
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "fine: definitely not > `u64::MAX` constructors"
        )]
        NonZero::new(self.size).is_some_and(|size| prng.rand() as usize % size != 0)
    }
}

impl fmt::Debug for Size {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <usize as fmt::Debug>::fmt(&self.size, f)
    }
}

impl fmt::Display for Size {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <usize as fmt::Display>::fmt(&self.size, f)
    }
}
