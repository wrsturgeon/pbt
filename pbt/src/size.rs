use {
    crate::{
        construct::{Construct, arbitrary},
        reflection::{AlgebraicTypeFormer, PrecomputedTypeFormer, Type, info_by_id, type_of},
    },
    core::{any::type_name, cmp, fmt, iter, num::NonZero},
    std::collections::{BTreeMap, BinaryHeap},
    wyrand::WyRand,
};

/// A non-`Clone` wrapper around `usize`
/// to prevent accounting errors.
#[derive(Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Size {
    /// The internal size value that must not be `Clone`d.
    size: usize,
}

pub struct Sizes {
    /// A map from (only inducive) types to
    /// sizes for each field of that type.
    map: BTreeMap<Type, Vec<Size>>,
}

impl Size {
    #[inline]
    pub fn expanding() -> impl Iterator<Item = Self> {
        (0..).map(|size| Self { size })
    }

    #[inline]
    pub fn partition<T: Construct>(self, ctor_idx: NonZero<usize>, prng: &mut WyRand) -> Sizes {
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
            return Sizes {
                map: BTreeMap::new(),
            };
        };
        #[expect(
            clippy::indexing_slicing,
            reason = "internal invariants; violation should panic"
        )]
        let (_ctor_fn, ref deps) = all_constructors[ctor_idx.get() - 1];

        // Count the number of inductive fields,
        // regardless of whether they're trivial wrappers
        // (e.g. `Box` is a trivial wrapper but `Box<...>` is still inductive):
        let mut n_ind = 0;
        for (&ty, count) in deps.constructor.immediate.iter() {
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "fields bounded by system hardware, defined to match the capacity of `usize`"
            )]
            // TODO: cache inductivity
            if info_by_id(ty).vertex.is_inductive() {
                n_ind += count.get();
            }
        }

        let Some(mut sizes) = self.partition_into(n_ind, prng, !info.trivial) else {
            return Sizes {
                map: BTreeMap::new(),
            };
        };

        // Use each size for an inductive type:
        let mut map = BTreeMap::<Type, Vec<Size>>::new();
        for (&ty, count) in deps.constructor.immediate.iter() {
            // TODO: cache inductivity
            if info_by_id(ty).vertex.is_inductive() {
                let v = map.entry(ty).or_default();
                for _ in 0..count.get() {
                    #[expect(
                        clippy::expect_used,
                        reason = "internal invariants; violation should panic"
                    )]
                    v.push(
                        sizes
                            .next()
                            .expect("internal `pbt` error: inductive type mis-count"),
                    );
                }
            }
        }
        Sizes { map }
    }

    /// Partition this total size into `n` sizes
    /// which add up to the original size,
    /// optionally minus one iff `minus_one`.
    /// # Panics
    /// If `size` is `usize::MAX` and `!minus_one`.
    #[inline]
    pub fn partition_into(
        self,
        n: usize,
        prng: &mut WyRand,
        minus_one: bool,
    ) -> Option<impl 'static + Iterator<Item = Self>> {
        // We want `n` sections, so we'll use the spaces between
        // the beginning, the end, and `n - 1` stars:
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
        let mut bars: BinaryHeap<cmp::Reverse<usize>> =
            iter::repeat_with(|| cmp::Reverse(prng.rand() as usize % n_inclusive))
                .take(n_bars)
                .collect();

        // Compute sizes by measuring the spaces between bars (+ beginning & end),
        // noting that the binary heap can efficiently drain in sorted order:
        let mut prev = 0;
        let mut end = Some(self.size.checked_sub(usize::from(minus_one))?);
        #[expect(clippy::arithmetic_side_effects, reason = "sorted")]
        Some(iter::from_fn(move || {
            let size = if let Some(cmp::Reverse(bar)) = bars.pop() {
                let difference = bar - prev;
                prev = bar;
                difference
            } else {
                end.take()? - prev
            };
            Some(Self { size })
        }))
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

    #[inline]
    #[must_use]
    pub fn zero() -> Self {
        Self { size: 0 }
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

impl Sizes {
    /// Generate an arbitrary term of type `T` using the
    /// size partitioned for it via `Size::partition`.
    /// # Panics
    /// If `T` is uninstantiable.
    #[inline]
    pub fn arbitrary<T: Construct>(&mut self, prng: &mut WyRand) -> T {
        let id = type_of::<T>();
        let size = self.map.get_mut(&id).map_or(Size { size: 0 }, |v| {
            #[expect(
                clippy::expect_used,
                reason = "internal invariants; violation should panic"
            )]
            v.pop()
                .expect("internal `pbt` error: inductive type mis-count")
        });
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
}
