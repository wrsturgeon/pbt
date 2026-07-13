//! `QuixBugs` adapter for Proptest.

use {
    core::fmt::Debug,
    proptest::{
        arbitrary::{Arbitrary, any},
        test_runner::{Config, TestCaseError, TestError, TestRunner},
    },
    quixbugs_api::Check,
};

/// The Proptest implementation of the benchmark interface.
#[non_exhaustive]
pub struct Proptest;

impl<T> Check<T> for Proptest
where
    T: Arbitrary + Debug,
{
    #[inline]
    #[expect(
        clippy::panic,
        reason = "An aborted runner violates the benchmark adapter contract."
    )]
    fn check(property: fn(&T) -> bool) -> Option<T> {
        let mut runner = TestRunner::new(config());
        match runner.run(&any::<T>(), |candidate| {
            if property(&candidate) {
                Ok(())
            } else {
                Err(TestCaseError::fail("counterexample"))
            }
        }) {
            Err(TestError::Fail(_reason, witness)) => Some(witness),
            Err(TestError::Abort(reason)) => panic!("Proptest aborted: {reason}"),
            Ok(()) => None,
        }
    }
}

/// Configure an unbounded search without Proptest's irrelevant persistence warning.
#[cfg_attr(test, mutants::skip)] // Configuration is policy rather than executable behavior.
fn config() -> Config {
    Config {
        cases: u32::MAX,
        failure_persistence: None,
        ..Config::default()
    }
}

#[cfg(test)]
mod tests {
    use {
        super::Proptest,
        core::sync::atomic::{AtomicUsize, Ordering},
        quixbugs_api::Check as _,
    };

    /// Count calls to a property whose failure is beyond Proptest's default case count.
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    /// Fail only after Proptest's default case count has been exhausted.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "Check properties receive their input by reference."
    )]
    fn late_failure(_: &usize) -> bool {
        CALLS.fetch_add(1, Ordering::Relaxed) < 300
    }

    /// Check that generation and shrinking return the minimal witness.
    #[test]
    fn minimizes_witness() {
        assert_eq!(Proptest::check(|value: &usize| *value < 2), Some(2));
    }

    /// Check that the adapter does not stop after Proptest's default case count.
    #[test]
    fn searches_until_failure() {
        CALLS.store(0, Ordering::Relaxed);
        assert!(Proptest::check(late_failure).is_some());
    }
}
