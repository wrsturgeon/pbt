//! Implementations for `Vec<_>`.

pub mod refine;

use {
    crate::{
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{corner::Corner, refine::Refine, rnd::Rnd, size::Size, weight::Weight},
    },
    core::{hint::unreachable_unchecked, iter},
};

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
    type Refine = refine::Iter<T>;
    #[inline]
    fn refine(&self, size: usize) -> Self::Refine {
        refine::Iter::new(self, size)
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

    #[test]
    fn refine_vec_of_vec() {
        let orig = vec![vec![], vec![()], vec![(), ()]];
        assert_eq!(orig.refine(0).next(), None);
        assert_eq!(orig.refine(1).next(), None);
        assert_eq!(orig.refine(2).next(), None);
        assert_eq!(orig.refine(3).next(), None);
        assert_eq!(orig.refine(4).next(), None);
        assert_eq!(orig.refine(5).next(), None);
        {
            let mut iter = orig.refine(6);
            assert_eq!(iter.next(), Some(vec![vec![], vec![()], vec![(), ()]]));
            assert_eq!(iter.next(), None);
        }
        assert_eq!(orig.refine(7).next(), None);
    }

    // TODO: enable
    /*
    impl_tests!(Vec<Infallible>, vec_void);
    impl_tests!(Vec<()>, vec_unit);
    impl_tests!(Vec<bool>, vec_bool); // TODO: remove and switch to the below
    // impl_tests!(Vec<u8>, vec_u8); // TODO
    impl_tests!(Vec<Vec<()>>, vec_vec_unit);

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

    #[inline]
    fn decimate_vec_of_vec() {
        let orig = vec![vec![], vec![()], vec![(), ()]];
        {
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(1);
            assert_eq!(iter.next(), Some(vec![vec![]]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(2);
            assert_eq!(iter.next(), Some(vec![vec![()]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![]]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(3);
            assert_eq!(iter.next(), Some(vec![vec![(), ()]]));
            assert_eq!(iter.next(), Some(vec![vec![()], vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![()]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![], vec![]]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(3);
            assert_eq!(iter.next(), Some(vec![vec![(), (), ()]]));
            assert_eq!(iter.next(), Some(vec![vec![(), ()], vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![()], vec![()]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![(), ()]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![()], vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![], vec![()]]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(4);
            assert_eq!(iter.next(), Some(vec![vec![(), ()], vec![()]]));
            assert_eq!(iter.next(), Some(vec![vec![()], vec![(), ()]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![()], vec![()]]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(5);
            // TODO
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(6);
            // TODO
            assert_eq!(iter.next(), None);
        }
        assert_eq!(orig.decimate(7).next(), None);
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
