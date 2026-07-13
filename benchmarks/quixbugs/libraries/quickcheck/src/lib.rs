//! `QuixBugs` adapter for `QuickCheck`.

use {
    core::fmt::Debug,
    quickcheck::{Arbitrary, Gen},
    quixbugs_api::Check,
};

/// The `QuickCheck` implementation of the benchmark interface.
#[non_exhaustive]
pub struct QuickCheck;

impl<T> Check<T> for QuickCheck
where
    T: Arbitrary + Clone + Debug,
{
    #[inline]
    fn check(property: fn(&T) -> bool) -> Option<T> {
        let mut generator = Gen::new(100);
        loop {
            let candidate = T::arbitrary(&mut generator);
            if !property(&candidate) {
                return Some(shrink(candidate, property));
            }
        }
    }
}

/// Follow `QuickCheck`'s first-failing-candidate shrinking strategy.
#[inline]
fn shrink<T>(mut witness: T, property: fn(&T) -> bool) -> T
where
    T: Arbitrary,
{
    loop {
        let Some(candidate) = witness.shrink().find(|candidate| !property(candidate)) else {
            return witness;
        };
        witness = candidate;
    }
}

#[cfg(test)]
mod tests {
    use {super::QuickCheck, quixbugs_api::Check as _};

    /// Check that generation and shrinking return the minimal witness.
    #[test]
    fn minimizes_witness() {
        assert_eq!(QuickCheck::check(|value: &usize| *value < 2), Some(2));
    }
}
