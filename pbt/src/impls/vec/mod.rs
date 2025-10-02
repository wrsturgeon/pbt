//! Implementations for `Vec<_>`.

// pub mod decimate;
pub mod refine;

use {
    crate::{
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{corner::Corner, rnd::Rnd, size::Size, weight::Weight},
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

// TODO: enable
/*
#[cfg(test)]
mod test {
    use {super::*, crate::impl_tests, core::convert::Infallible};

    impl_tests!(Vec<Infallible>, vec_void);
    impl_tests!(Vec<()>, vec_unit);
    impl_tests!(Vec<bool>, vec_bool); // TODO: remove and switch to the below
    // impl_tests!(Vec<u8>, vec_u8); // TODO
    impl_tests!(Vec<Vec<()>>, vec_vec_unit);
}
*/
