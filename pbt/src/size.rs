use {
    crate::{
        pbt::{MaybeUninstantiable, Pbt, arbitrary, try_arbitrary},
        reflection::{AlgebraicTypeFormer, PrecomputedTypeFormer, Type, info_by_id, type_of},
    },
    alloc::collections::BinaryHeap,
    core::{any::type_name, cmp, fmt, iter, num::NonZero},
    wyrand::WyRand,
};

/// A non-`Clone` wrapper around `usize`
/// to prevent accounting errors.
#[derive(Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Size {
    /// The internal size value that must not be `Clone`d.
    size: usize,
}

/// One size for each field of each big type.
/// Compute sizes by measuring the spaces between bars (+ beginning & end),
/// noting that a binary heap can efficiently drain in sorted order.
#[derive(Debug, Default)]
pub struct Sizes {
    /// All "bars" between "stars" except the
    /// beginning and end of the range itself.
    bars: BinaryHeap<cmp::Reverse<usize>>,
    /// The end of the range itself,
    /// unless that "bar" has already been used,
    /// in which case this is `None` to halt.
    end: Option<NonZero<usize>>,
    /// The most recent "bar" to be popped.
    /// This is initialized to zero, which
    /// represents the left edge of the range itself.
    prev: usize,
}

impl Size {
    /// Copy this size for retrying a failed generation attempt.
    #[inline]
    pub(crate) fn copy_for_retry(&self) -> Self {
        Self { size: self.size }
    }

    /// Return the standard monotonically expanding stream of generation sizes.
    #[inline]
    pub fn expanding() -> impl Iterator<Item = Self> {
        (0_usize..).map(|squared_size| Self {
            size: squared_size.isqrt(),
        })
    }

    /// Increase this size by one.
    #[inline]
    pub(crate) fn increment(&mut self) {
        #![expect(
            clippy::arithmetic_side_effects,
            reason = "Extremely rare: should panic."
        )]
        self.size += 1;
    }

    /// Partition this size across the inductive fields of constructor `ctor_idx` of `T`.
    #[inline]
    pub fn partition<T>(self, ctor_idx: NonZero<usize>, prng: &mut WyRand) -> Sizes
    where
        T: Pbt,
    {
        self.partition_by_id(type_of::<T>(), ctor_idx, prng)
    }

    /// Use a stars-and-bars-like method to partition a total size
    /// into sizes for each inductive type in its multiset of fields,
    /// minus one for this node itself iff not a trivial wrapper.
    #[inline]
    fn partition_by_id(self, id: Type, ctor_idx: NonZero<usize>, prng: &mut WyRand) -> Sizes {
        let info = info_by_id(id);
        let PrecomputedTypeFormer::Algebraic(AlgebraicTypeFormer {
            ref all_constructors,
            ..
        }) = info.type_former
        else {
            return Sizes::default();
        };
        // SAFETY: By the correct implementation of `eliminator`
        // (i.e., by macro logic plus the few implementations in this crate).
        #[expect(clippy::multiple_unsafe_ops_per_block, reason = "logically grouped")]
        let (_ctor_fn, ref deps) =
            *unsafe { all_constructors.get_unchecked(ctor_idx.get().unchecked_sub(1)) };

        // Count the number of inductive fields,
        // regardless of whether they're trivial wrappers
        // (e.g. `Box` is a trivial wrapper but `Box<...>` is still inductive):
        let mut n_ind = 0;
        for (&ty, count) in deps.constructor.immediate.iter() {
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "fields bounded by system hardware, defined to match the capacity of `usize`"
            )]
            if info_by_id(ty).vertex.is_inductive() {
                n_ind += count.get();
            }
        }

        self.partition_into(n_ind, prng, !info.trivial)
    }

    /// Partition this total size into `n` sizes
    /// which add up to the original size,
    /// optionally minus one iff `minus_one`.
    /// # Panics
    /// If `size` is `usize::MAX` and `!minus_one`.
    #[inline]
    pub(crate) fn partition_into(&self, n: usize, prng: &mut WyRand, minus_one: bool) -> Sizes {
        self.try_partition_into(n, prng, minus_one)
            .unwrap_or_default()
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
        NonZero::new(self.size.isqrt())
            .is_some_and(|denominator| prng.rand() as usize % denominator != 0)
    }

    /// Partition this total size into `n` sizes
    /// which add up to the original size,
    /// optionally minus one iff `minus_one`.
    /// # Panics
    /// If `size` is `usize::MAX` and `!minus_one`.
    #[inline]
    pub(crate) fn try_partition_into(
        &self,
        n: usize,
        prng: &mut WyRand,
        minus_one: bool,
    ) -> Option<Sizes> {
        let end = NonZero::new(self.size.checked_sub(usize::from(minus_one))?)?;

        // We want `n` sections, so we'll use the spaces between
        // the beginning, the end, and `n - 1` bars:
        let n_bars = n.checked_sub(1)?;

        // If this is a trivial wrapper and/or non-inductive type,
        // don't account for it while tracking full AST size;
        // otherwise, this AST node counts, so we should
        // decrement the remaining size for the rest of the structure.
        let n_inclusive = NonZero::new(
            #[expect(
                clippy::expect_used,
                clippy::unwrap_in_result,
                reason = "internal invariants; violation should panic"
            )]
            self.size
                .checked_add(usize::from(!minus_one))
                .expect("internal `pbt` error: size of `usize::MAX`"),
        )?;
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "fine: definitely not > `u64::MAX` fields"
        )]
        let bars: BinaryHeap<cmp::Reverse<usize>> =
            iter::repeat_with(|| cmp::Reverse(prng.rand() as usize % n_inclusive))
                .take(n_bars)
                .collect();

        Some(Sizes {
            bars,
            prev: 0,
            end: Some(end),
        })
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

impl Iterator for Sizes {
    type Item = Size;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let size = if let Some(cmp::Reverse(bar)) = self.bars.pop() {
            // SAFETY: by the sorted invariant of `BinaryHeap`
            let difference = unsafe { bar.unchecked_sub(self.prev) };
            self.prev = bar;
            difference
        } else {
            // SAFETY: by the sorted invariant of `BinaryHeap`
            unsafe { self.end.take()?.get().unchecked_sub(self.prev) }
        };
        Some(Size { size })
    }
}

impl Sizes {
    /// Generate an arbitrary term of type `T` using the
    /// size partitioned for it via `Size::partition`.
    /// # Panics
    /// If `T` is uninstantiable.
    #[inline]
    pub fn arbitrary<T>(&mut self, prng: &mut WyRand) -> T
    where
        T: Pbt,
    {
        let ty = type_of::<T>();
        let info = info_by_id(ty);
        let size = if info.is_big() {
            self.next().unwrap_or_default()
        } else {
            Size { size: 0 }
        };
        #[expect(clippy::todo, reason = "TODO")]
        let Some(t) = arbitrary::<T>(prng, size) else {
            todo!(
                "uninstantiable type `{}` in `{}`",
                type_name::<T>(),
                type_name::<Self>(),
            );
        };
        t
    }

    /// Discard all unused field sizes after abandoning constructor generation.
    #[inline]
    pub(crate) fn discard_remaining(&mut self) {
        while self.next().is_some() {}
    }

    /// Try to generate an arbitrary term of type `T` using the
    /// size partitioned for it via `Size::partition`.
    /// # Errors
    /// Returns [`MaybeUninstantiable::Retry`] when rejection sampling could not
    /// decide at this size, or [`MaybeUninstantiable::Uninstantiable`] when `T`
    /// has no structurally available constructor.
    #[inline]
    pub fn try_arbitrary<T>(&mut self, prng: &mut WyRand) -> Result<T, MaybeUninstantiable>
    where
        T: Pbt,
    {
        let ty = type_of::<T>();
        let info = info_by_id(ty);
        let size = if info.is_big() {
            self.next().unwrap_or_default()
        } else {
            Size { size: 0 }
        };
        try_arbitrary::<T>(prng, size)
    }
}

impl Drop for Sizes {
    #[inline]
    fn drop(&mut self) {
        debug_assert_eq!(
            self.next(),
            None,
            "internal `pbt` error: inductive type mis-count",
        );
    }
}
