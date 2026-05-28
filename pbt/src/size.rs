//! Approximate AST size of a value to be generated,
//! counting only inductive types and ignoring leaves.

use {
    alloc::collections::BinaryHeap,
    core::{cmp, iter, mem, num::NonZero},
    wyrand::WyRand,
};

/// Partition this size into a known number of sizes
/// which add up to the same size we started with.
pub struct Partition {
    /// The "bars" in "stars and bars."
    /// (The *combinatorics* "stars and bars," not the confederate flag.)
    separators: Option<BinaryHeap<cmp::Reverse<usize>>>,
    /// The original size being partitioned.
    total: usize,
    /// The total size that has been used so far,
    /// e.g. if we're mid-iteration over partitioned sizes.
    used: usize,
}

/// Approximate AST size of a value to be generated,
/// counting only inductive types and ignoring leaves.
///
/// N.B.: this type is *not* `Clone` and
/// must not be, since size cannot be reused.
/// Instead, size must merely be split/partitioned
/// among fields of a chosen variant,
/// all the way down to leaves (forced when size runs out).
#[derive(Default)]
pub struct Size {
    /// Approximate AST size of a value to be generated,
    /// counting only inductive types and ignoring leaves.
    ///
    /// N.B.: this field is *not* public and
    /// must not be accessible, since size cannot be reused.
    /// Instead, size must merely be split/partitioned
    /// among fields of a chosen variant,
    /// all the way down to leaves (forced when size runs out).
    total: usize,
}

impl Size {
    /// Partition this size into a known number of sizes
    /// which add up to the same size we started with.
    ///
    /// # Panics
    ///
    /// If the total size of `self` is `usize::MAX`.
    /// This is a bad idea, since your
    /// memory will not hold the generated value.
    #[inline]
    pub fn partition(self, into_how_many: usize, prng: &mut WyRand) -> Partition {
        // TODO: switch algorithm if `into_how_many < self.total`
        let separators = into_how_many.checked_sub(1).map(|n_separators| {
            #[expect(
                clippy::expect_used,
                reason = "genuinely cataclysmic state, should panic"
            )]
            let incremented = self
                .total
                .checked_add(1)
                .expect("PSA from `pbt`: your memory will not hold a term of size `usize::MAX`.");
            // SAFETY: Incremented above, starting from at least zero,
            // so the result must be at least 1.
            let modulo = unsafe { NonZero::new_unchecked(incremented) };
            #[expect(
                clippy::as_conversions,
                clippy::cast_possible_truncation,
                reason = "intentional"
            )]
            iter::repeat_with(|| cmp::Reverse(prng.rand() as usize % modulo))
                .take(n_separators)
                .collect()
        });
        Partition {
            total: self.total,
            used: 0,
            separators,
        }
    }

    /// Based on the size we have left, should we
    /// head toward a leaf or recurse again?
    #[inline]
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "OK: `u64` is already huge"
    )]
    pub fn should_recurse(&self, prng: &mut WyRand) -> bool {
        let Some(denominator) = NonZero::new(self.total) else {
            return false;
        };
        (prng.rand() as usize % denominator) != 0
    }

    /// A total size of zero.
    #[inline]
    #[must_use]
    pub const fn zero() -> Self {
        Self { total: 0 }
    }
}

impl Partition {
    /// A partition of zero size into zero parts.
    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            separators: None,
            total: 0,
            used: 0,
        }
    }
}

impl Iterator for Partition {
    type Item = Size;

    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::unwrap_in_result,
        reason = "For internal use only: invariant violations should fail loudly."
    )]
    fn next(&mut self) -> Option<Self::Item> {
        let separators = self
            .separators
            .as_mut()
            .expect("INTERNAL ERROR (`pbt`): overdrawn size partition");
        let cap = separators.pop().map_or(self.total, |cmp::Reverse(u)| u);
        let used = mem::replace(&mut self.used, cap);
        Some(Size {
            // SAFETY: by sorted-`pop` invariant of `BinaryHeap`.
            total: unsafe { cap.unchecked_sub(used) },
        })
        // This will continue to generate 0 ad aeternum
        // after the heap has been exhausted.
    }
}

impl Drop for Partition {
    #[inline]
    fn drop(&mut self) {
        debug_assert_eq!(
            self.used, self.total,
            "INTERNAL ERROR (`pbt`): unused size partition",
        );
    }
}

#[cfg(test)]
mod test {
    use {super::*, pretty_assertions::assert_eq};

    #[test]
    fn partition_10() {
        let mut prng = WyRand::new(0xBAAD_5EED_BAAD_C0DE);
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .take(3)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![1, 8, 1],
        );
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .take(3)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![5, 3, 2],
        );
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .take(10)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![6, 3, 1, 0, 0, 0, 0, 0, 0, 0],
        );
    }
}
