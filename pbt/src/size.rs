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
#[derive(Debug, Default)]
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
    #[expect(
        clippy::expect_used,
        reason = "The hardware will die before the test index overflows."
    )]
    pub(crate) fn increasing() -> impl Iterator<Item = Self> {
        let mut index = 0_usize;
        let mut next_square = 1_usize;
        let mut root = 0_usize;

        iter::from_fn(move || {
            if index == next_square {
                root = root
                    .checked_add(1)
                    .expect("PSA from `pbt`: your hardware cannot run this many tests.");
                let next_root = root
                    .checked_add(1)
                    .expect("PSA from `pbt`: your hardware cannot run this many tests.");
                next_square = next_root
                    .checked_mul(next_root)
                    .expect("PSA from `pbt`: your hardware cannot run this many tests.");
            }
            let size = Self { total: root };
            index = index
                .checked_add(1)
                .expect("PSA from `pbt`: your hardware cannot run this many tests.");
            Some(size)
        })
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
        let Some(decremented) = self.total.checked_sub(1) else {
            return Partition {
                total: 0,
                used: 0,
                separators: Some(iter::repeat_n(cmp::Reverse(0), into_how_many).collect()),
            };
        };
        #[expect(clippy::integer_division, reason = "intentional")]
        let () = { self.total = decremented / branching_factor };

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
    fn next(&mut self) -> Option<Self::Item> {
        let separators = self.separators.as_mut()?;
        let cap = if let Some(cmp::Reverse(u)) = separators.pop() {
            u
        } else {
            self.separators = None;
            self.total
        };
        let used = mem::replace(&mut self.used, cap);
        Some(Size {
            // SAFETY: by sorted-`pop` invariant of `BinaryHeap`.
            total: unsafe { cap.unchecked_sub(used) },
        })
    }
}

impl Drop for Partition {
    #[inline]
    fn drop(&mut self) {
        debug_assert_eq!(
            self.used, self.total,
            "INTERNAL ERROR (`pbt`): unused size partition (`self.separators = {:?}`)",
            self.separators,
        );
    }
}

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{DEFAULT_N_CASES, getrandom},
        pretty_assertions::assert_eq,
    };

    #[test]
    fn increasing_takes_square_root() {
        assert_eq!(
            Size::increasing()
                .take(10)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![0, 1, 1, 1, 2, 2, 2, 2, 2, 3],
        );
    }

    #[test]
    fn partition_adds_up_to_original_over_branching_factor() {
        let mut prng = WyRand::new(getrandom());
        for size in Size::increasing().take(DEFAULT_N_CASES) {
            let total = size.total;
            #[expect(
                clippy::as_conversions,
                clippy::cast_possible_truncation,
                reason = "usize::MAX > 10"
            )]
            #[expect(clippy::integer_division_remainder_used, reason = "10 > 0")]
            let into_how_many = 1 + (prng.rand() as usize % 10);
            #[expect(
                clippy::integer_division,
                clippy::integer_division_remainder_used,
                reason = "intentional"
            )]
            let expected = total.saturating_sub(1) / into_how_many;
            let partitioned: Vec<Size> = size.partition(into_how_many, &mut prng).collect();
            let actual: usize = partitioned.iter().map(|s| s.total).sum();
            assert_eq!(
                actual, expected,
                "{expected} -> {partitioned:?} -> {actual} =/= {expected}",
            );
        }
    }

    #[test]
    fn deterministic_partition() {
        let mut prng = WyRand::new(42); // deterministic
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![1, 1, 1],
        );
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![0, 3, 0],
        );
        assert_eq!(
            Size { total: 10 }
                .partition(3, &mut prng)
                .map(|Size { total }| total)
                .collect::<Vec<usize>>(),
            vec![1, 0, 2],
        );
    }

    #[test]
    #[should_panic(
        expected = "INTERNAL ERROR (`pbt`): unused size partition (`self.separators = Some([])`)"
    )]
    fn unused_partition() {
        let mut prng = WyRand::new(42); // deterministic
        let mut partition = Size { total: 10 }
            .partition(3, &mut prng)
            .map(|Size { total }| total);
        assert_eq!(partition.next(), Some(1));
        assert_eq!(partition.next(), Some(1));
        // dropped without using the third item
    }
}
