//! Approximate AST size of a value to be generated,
//! counting only inductive types and ignoring leaves.

use {
    alloc::collections::BinaryHeap,
    core::{cmp, iter, mem, num::NonZero},
    wyrand::WyRand,
};

/// Partition this size into a known number of sizes
/// which add up to the same size we started with.
pub(crate) struct Partition {
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
pub(crate) struct Size {
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
    /// An infinite iterator of increasing sizes,
    /// starting from zero.
    #[inline]
    pub(crate) fn increasing() -> impl Iterator<Item = Self> {
        (0..).map(|total| Self { total })
    }

    /// Partition this size into a known number of sizes
    /// which add up to the same size we started with.
    ///
    /// # Panics
    ///
    /// If the total size of `self` is `usize::MAX`.
    /// This is a bad idea, since your
    /// memory will not hold the generated value.
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    pub(crate) fn partition(mut self, into_how_many: usize, prng: &mut WyRand) -> Partition {
        let Some(branching_factor) = NonZero::new(into_how_many) else {
            return Partition::empty();
        };
        self.total /= branching_factor;

        // TODO: switch algorithm if `into_how_many < self.total`

        let incremented = self
            .total
            .checked_add(1)
            .expect("PSA from `pbt`: your memory will not hold a term of size `usize::MAX`.");
        // SAFETY: Incremented above, starting from at least zero,
        // so the result must be at least 1.
        let modulo = unsafe { NonZero::new_unchecked(incremented) };

        // SAFETY: Nonzero. Checked above.
        let n_separators = unsafe { into_how_many.unchecked_sub(1) };
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "intentional"
        )]
        let separators = Some({
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
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    pub(crate) fn should_recurse(&self, prng: &mut WyRand) -> bool {
        // SAFETY: Incremented and didn't overflow.
        let incremented =
            unsafe {
                NonZero::new_unchecked(self.total.checked_add(1).expect(
                    "PSA from `pbt`: your memory will not hold a term of size `usize::MAX`.",
                ))
            };

        (prng.rand() as usize % incremented) != 0
    }

    /// A total size of zero.
    #[inline]
    #[must_use]
    pub(crate) const fn zero() -> Self {
        Self { total: 0 }
    }
}

impl Partition {
    /// A partition of zero size into zero parts.
    #[inline]
    #[must_use]
    const fn empty() -> Self {
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
        reason = "Internal invariants: violations should fail loudly."
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
            vec![0, 2, 1],
        );
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .take(3)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![1, 1, 1],
        );
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .take(10)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![0, 2, 1, 0, 0, 0, 0, 0, 0, 0],
        );
    }
}
