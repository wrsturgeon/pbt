//! Implementations for `Vec<_>`.

use {
    crate::{
        iter::Cache,
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{corner::Corner, refine::Refine, rnd::Rnd, size::Size, weight::Weight},
    },
    core::{hint::unreachable_unchecked, iter, ptr},
};

/// Refine a slice of values,
/// returning each refinement as a `Vec<_>`.
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum Refiner<T: Clone + Refine> {
    /// Non-empty slice, split into its head and tail.
    Cons {
        /// The size of refinements to the first element.
        /// This is initialized to the maximum possible size
        /// (after accounting for the length of the tail)
        /// then decremented down to zero, then set to `None`
        /// to indicate that this iterator is finished.
        head_size: Option<usize>,
        /// Caching iterator over refinements to the first element,
        /// each of which is of size `head_size` (if any).
        head: Option<Cache<T::Refine>>,
        /// Iterator over the rest of the slice (same logic as here).
        tail: Box<Self>,
        /// The original value of the first element,
        /// for use when refining to a new size.
        original: T,
    },
    /// Empty slice.
    Nil {
        /// Remaining size that has not been refined by preceding elements.
        /// Note that `Iterator::next()` will produce `Some(_)`
        /// if and only if this field is `Some(0)`
        /// (meaning that the total size is exactly right),
        /// and upon doing so, this will be set to `None`.
        remaining_size: Option<usize>,
    },
}

impl<T: Clone + Refine> Refiner<T> {
    /// Increase the refinement size of the first element,
    /// clearing the iterator if any (which would have produced an outdated size).
    #[inline]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "If a `usize` overflows, crashing is probably the best option."
    )]
    pub fn increment_size(&mut self) {
        match *self {
            Self::Nil {
                ref mut remaining_size,
            } => *remaining_size = Some(remaining_size.map_or(0, |size| size + 1)),
            Self::Cons {
                ref mut head_size,
                ref mut head,
                ..
            } => {
                *head_size = Some(head_size.map_or(0, |size| size + 1));
                *head = None;
            }
        }
    }

    /// Prepare to refine this slice.
    #[inline]
    pub fn new(slice: &[T], size: usize) -> Self {
        match *slice {
            [] => Self::Nil {
                remaining_size: Some(size),
            },
            [ref head, ref tail @ ..] => Self::Cons {
                head_size: size.checked_sub(slice.len()),
                head: None,
                tail: Box::new(Self::new_with_size_zero(tail)),
                original: head.clone(),
            },
        }
    }

    /// Prepare to refine this slice, assigning each element a size of `Some(0)`.
    #[inline]
    fn new_with_size_zero(slice: &[T]) -> Self {
        match *slice {
            [] => Self::Nil {
                remaining_size: Some(0),
            },
            [ref head, ref tail @ ..] => Self::Cons {
                head_size: Some(0),
                head: None,
                tail: Box::new(Self::new_with_size_zero(tail)),
                original: head.clone(),
            },
        }
    }

    /// Build a vector incrementally instead of appending `O(n)` times
    /// (which would have brought the total runtime to `O(n^2)`).
    #[inline]
    pub fn next_acc(&mut self, acc: &mut Vec<T>) -> Option<()> {
        match *self {
            Self::Nil {
                ref mut remaining_size,
            } => {
                let opt = matches!(remaining_size, Some(0)).then_some(());
                *remaining_size = None;
                opt
            }
            Self::Cons {
                ref mut head_size,
                ref mut head,
                ref mut tail,
                ref original,
            } => 'head_sizes: loop {
                let current_head_size = (*head_size)?;
                loop {
                    let current_head_iter = head
                        .get_or_insert_with(move || Cache::new(original.refine(current_head_size)));
                    let Some(current_head) = current_head_iter.next() else {
                        *head_size = current_head_size.checked_sub(1);
                        // SAFETY: We know that `head` is `Some(..)`, so we can
                        // drop the value without checking if it's `None,
                        // then overwrite it without dropping it a second time.
                        #[expect(
                            clippy::multiple_unsafe_ops_per_block,
                            reason = "Logically connected."
                        )]
                        unsafe {
                            let () = ptr::drop_in_place(current_head_iter);
                            let () = ptr::write(head, None);
                        }
                        let () = tail.increment_size();
                        continue 'head_sizes;
                    };
                    let () = acc.push(current_head);
                    if matches!(tail.next_acc(acc), Some(())) {
                        return Some(());
                    }
                    // SAFETY: We just pushed, and `next_acc` never pops more than it pushes.
                    let () = drop::<T>(unsafe { acc.pop().unwrap_unchecked() });
                    let () = current_head_iter.clear();
                    // TODO: Maybe make the above a bit tighter by jumping up in the loop
                    // rather than starting over entirely?
                }
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<T: Clone + Refine> Iterator for Refiner<T> {
    type Item = Vec<T>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut acc = vec![];
        let () = self.next_acc(&mut acc)?;
        Some(acc)
    }
}

impl<T: Weight> Weight for Vec<T> {
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> =
        <[T] as Weight>::MAX_EXPECTED_WEIGHT;
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> = <[T] as Weight>::MAX_WEIGHT;
    #[inline]
    fn weight(&self) -> usize {
        <[T] as Weight>::weight(self)
    }
}

impl<T: Size> Size for Vec<T> {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> =
        <[T] as Size>::MAX_SIZE;
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        <[T] as Size>::size(self)
    }
}

impl<T: Corner> Corner for Vec<T> {
    type Corners = iter::Chain<iter::Once<Self>, iter::Map<T::Corners, fn(T) -> Self>>;
    #[inline]
    fn corners() -> Self::Corners {
        iter::once(vec![]).chain(T::corners().map(
            #[expect(
                clippy::as_conversions,
                reason = "Function pointer conversions are checked more thoroughly"
            )]
            {
                (|singleton| vec![singleton]) as fn(_) -> _
            },
        ))
    }
}

impl<T: Rnd> Rnd for Vec<T> {
    #[inline]
    fn rnd<Rng: rand_core::RngCore>(
        rng: &mut Rng,
        expected_weight: f32,
    ) -> MaybeInstantiable<Self> {
        // There are basically two ways to make a big vector:
        // make a *long* vector or make a vector with *huge elements*.
        // We want to adjust how far we lean either way:
        // one run might produce a long vector, and the next might use large elements,
        // rather than splitting the difference on every run
        // (in which case it would be very unlikely to observe either "shape").
        // So we use a stars-and-bars-style partition to represent the trade-off explicitly.
        // Note that, if each element has an expected weight `E` and the length is `L`,
        // then the total weight is `L + (L * E) = L(1 + E)` (since each element incurs one point).
        // So the "fairest" allocation would give both `L` and `E` approximately
        // the square root of the total weight each.
        #[expect(clippy::modulo_arithmetic, reason = "intentional")]
        let mean_element_weight = match T::MAX_EXPECTED_WEIGHT {
            MaybeInstantiable::Uninstantiable => {
                return MaybeInstantiable::Instantiable(vec![]);
            }
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
                let rnd = f32::from_bits(rng.next_u32());
                let sqrt = rnd % f32::sqrt(expected_weight);
                sqrt * sqrt
            }
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(max)) => {
                let rnd = f32::from_bits(rng.next_u32());
                let sqrt = rnd % f32::sqrt(expected_weight);
                let in_range = sqrt * sqrt;
                in_range.min(max)
            }
        };
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "intentional"
        )]
        let length = (expected_weight / (mean_element_weight + 1.)) as usize;

        let mut acc: Self = Self::with_capacity(length);
        let () = acc.resize_with(length, move || {
            let MaybeInstantiable::Instantiable(element) = T::rnd(rng, mean_element_weight) else {
                // SAFETY: If `T` were uninstantiable, the above `match` would have exited.
                unsafe { unreachable_unchecked() }
            };
            element
        });
        MaybeInstantiable::Instantiable(acc)
    }
}

/*
impl<T: Decimate> Decimate for Vec<T> {
    type Decimate = Decimator<T>;
    #[inline]
    fn decimate(&self, weight: usize) -> Self::Decimate {
        Decimator::new(self, weight)
    }
}
*/

impl<T: Clone + Refine> Refine for Vec<T> {
    type Refine = Refiner<T>;
    #[inline]
    fn refine(&self, size: usize) -> Self::Refine {
        Refiner::new(self, size)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn refine_vec_false_true() {
        let orig = vec![false, true];
        {
            let mut iter = orig.refine(0);
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(1);
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(2);
            assert_eq!(iter.next(), Some(vec![false, false]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(3);
            assert_eq!(iter.next(), Some(vec![false, true]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(4);
            assert_eq!(iter.next(), None);
        }
    }

    // TODO: enable
    /*
    impl_tests!(Vec<Infallible>, vec_void);
    impl_tests!(Vec<()>, vec_unit);
    impl_tests!(Vec<bool>, vec_bool); // TODO: remove and switch to the below
    // impl_tests!(Vec<u8>, vec_u8); // TODO

    #[test]
    fn decimate_vec_false_true() {
        let orig = vec![false, true];
        {
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(1);
            assert_eq!(iter.next(), Some(vec![false]));
            assert_eq!(iter.next(), Some(vec![true]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(2);
            assert_eq!(iter.next(), Some(vec![false, true]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(3);
            assert_eq!(iter.next(), None);
        }
    }

    #[test]
    fn decimate_vec_1234() {
        let orig = vec![1, 2, 3, 4_u8];
        {
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(1);
            assert_eq!(iter.next(), Some(vec![1]));
            assert_eq!(iter.next(), Some(vec![2]));
            assert_eq!(iter.next(), Some(vec![3]));
            assert_eq!(iter.next(), Some(vec![4]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(2);
            assert_eq!(iter.next(), Some(vec![1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 4]));
            assert_eq!(iter.next(), Some(vec![2, 3]));
            assert_eq!(iter.next(), Some(vec![2, 4]));
            assert_eq!(iter.next(), Some(vec![3, 4]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(3);
            assert_eq!(iter.next(), Some(vec![1, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 3, 4]));
            assert_eq!(iter.next(), Some(vec![2, 3, 4]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(4);
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 4]));
            assert_eq!(iter.next(), None);
        }
    }

    #[test]
    #[expect(
        clippy::cognitive_complexity,
        clippy::too_many_lines,
        reason = "Just a long iterator."
    )]
    fn refine_vec_1234() {
        let orig = vec![1, 2, 3, 4_u8];
        assert_eq!(orig.refine(0).next(), None);
        assert_eq!(orig.refine(1).next(), None);
        assert_eq!(orig.refine(2).next(), None);
        assert_eq!(orig.refine(3).next(), None);
        {
            let mut iter = orig.refine(4);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(5);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 1]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 0]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(6);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 2]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 1]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 1]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 0]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(7);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 3]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 2]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 1]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 2]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 1]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 1]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 0]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 2]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 1]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 1, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 1, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![3, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(8);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 4]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 3]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 2]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 3]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 2]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 1]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 2]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 1]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 3]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 2]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 1]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 2]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(9);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 5]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 4]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 3]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 5, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 4]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 3]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 2]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 3]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 2]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 1]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 1]));
            // ...
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 4]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 2]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 3]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 1]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 2]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 1]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 1]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 3]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(10);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 1, 5]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 4]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 0, 5]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 4]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 3]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 5, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 4]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 3]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 2]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 3]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 0, 0, 5]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 4]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 5, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 4]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 2]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 1]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 2]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 4]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(11);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 2, 5]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 1, 5]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 4]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 0, 5]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 4]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 3]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 5, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 4]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 0, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 1, 5]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 0, 5]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 4]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 5, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 4]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 2]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 4, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 3]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 5]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(12);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 8]));
            // ...
            // assert_eq!(iter.next(), Some(vec![0, 1, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 2, 5]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 1, 5]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 4]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 5]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 0, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 2, 5]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 1, 5]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 0, 5]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 4]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 5, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 4]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 6]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(13);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 9]));
            // ...
            // assert_eq!(iter.next(), Some(vec![0, 2, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 2, 5]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 6]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 1, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 2, 5]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 1, 5]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 5]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 7]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(14);
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 4]));
            assert_eq!(iter.next(), None);
        }
    }
    */
}
