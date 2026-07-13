//! `QuixBugs` adapter for `pbt`.

use quixbugs_api::Check;

/// The `pbt` implementation of the benchmark interface.
#[non_exhaustive]
pub struct Pbt;

impl<T> Check<T> for Pbt
where
    T: pbt::Pbt,
{
    #[inline]
    fn check(property: fn(&T) -> bool) -> Option<T> {
        let mut prng = pbt::WyRand::new(pbt::getrandom());
        pbt::witness_without_persistence(
            |candidate| (!property(candidate)).then_some(()),
            usize::MAX,
            &mut prng,
        )
        .map(|(witness, ())| witness)
    }
}

#[cfg(test)]
mod tests {
    use {super::Pbt, quixbugs_api::Check as _};

    /// Check that generation and shrinking return the minimal witness.
    #[test]
    fn minimizes_witness() {
        assert_eq!(Pbt::check(|value: &usize| *value < 2), Some(2));
    }
}
