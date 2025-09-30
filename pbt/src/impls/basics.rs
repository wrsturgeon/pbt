use {
    crate::{
        either::Either,
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{
            corner::Corner, decimate::Decimate, refine::Refine, rnd::Rnd, size::Size,
            weight::Weight,
        },
    },
    core::{array, convert::Infallible, hint::unreachable_unchecked, iter, option, slice},
};

impl Weight for Infallible {
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> =
        MaybeInstantiable::Uninstantiable;
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> = MaybeInstantiable::Uninstantiable;
    #[inline]
    fn weight(&self) -> usize {
        // SAFETY: Uninstantiable type.
        unsafe { unreachable_unchecked() }
    }
}

impl Size for Infallible {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> =
        MaybeInstantiable::Uninstantiable;
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        // SAFETY: Uninstantiable type.
        unsafe { unreachable_unchecked() }
    }
}

impl Corner for Infallible {
    type Corners = iter::Empty<Self>;
    #[inline]
    fn corners() -> Self::Corners {
        iter::empty()
    }
}

impl Rnd for Infallible {
    #[inline]
    fn rnd<Rng: rand_core::RngCore>(
        _rng: &mut Rng,
        _expected_weight: f32,
    ) -> MaybeInstantiable<Self> {
        MaybeInstantiable::Uninstantiable
    }
}

impl Decimate for Infallible {
    type Decimate = iter::Empty<Self>;
    #[inline]
    fn decimate(&self, _weight: usize) -> Self::Decimate {
        iter::empty()
    }
}

impl Refine for Infallible {
    type Refine = iter::Empty<Self>;
    #[inline]
    fn refine(&self, _size: usize) -> Self::Refine {
        iter::empty()
    }
}

impl Weight for () {
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0.));
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0));
    #[inline]
    fn weight(&self) -> usize {
        0
    }
}

impl Size for () {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(0)));
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(0)
    }
}

impl Corner for () {
    type Corners = iter::Once<Self>;
    #[inline]
    fn corners() -> Self::Corners {
        iter::once(())
    }
}

impl Rnd for () {
    #[inline]
    fn rnd<Rng: rand_core::RngCore>(
        _rng: &mut Rng,
        _expected_weight: f32,
    ) -> MaybeInstantiable<Self> {
        MaybeInstantiable::Instantiable(())
    }
}

impl Decimate for () {
    type Decimate = option::IntoIter<Self>;
    #[inline]
    fn decimate(&self, weight: usize) -> Self::Decimate {
        (weight == 0).then_some(()).into_iter()
    }
}

impl Refine for () {
    type Refine = option::IntoIter<Self>;
    #[inline]
    fn refine(&self, size: usize) -> Self::Refine {
        (size == 0).then_some(()).into_iter()
    }
}

impl Weight for bool {
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0.));
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0));
    #[inline]
    fn weight(&self) -> usize {
        0
    }
}

impl Size for bool {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(0)));
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(usize::from(*self))
    }
}

impl Corner for bool {
    type Corners = array::IntoIter<Self, 2>;
    #[inline]
    fn corners() -> Self::Corners {
        [false, true].into_iter()
    }
}

impl Rnd for bool {
    #[inline]
    fn rnd<Rng: rand_core::RngCore>(
        rng: &mut Rng,
        _expected_weight: f32,
    ) -> MaybeInstantiable<Self> {
        MaybeInstantiable::Instantiable((rng.next_u32() & 1) != 0)
    }
}

impl Decimate for bool {
    type Decimate = iter::Copied<slice::Iter<'static, Self>>;
    #[inline]
    fn decimate(&self, weight: usize) -> Self::Decimate {
        let len = if weight == 0 {
            if *self { 2 } else { 1 }
        } else {
            0
        };
        #[expect(
            clippy::indexing_slicing,
            reason = "this is a slam-dunk for the compiler"
        )]
        [false, true][..len].iter().copied()
    }
}

impl Refine for bool {
    type Refine = option::IntoIter<Self>;
    #[inline]
    fn refine(&self, size: usize) -> Self::Refine {
        match size {
            0 => Some(false),
            1 => self.then_some(true),
            _ => None,
        }
        .into_iter()
    }
}

impl<T: Weight> Weight for Option<T> {
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> = match T::MAX_EXPECTED_WEIGHT
    {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0.))
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite)) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite + 1.))
        }
    };
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> = match T::MAX_WEIGHT {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0))
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite)) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite + 1))
        }
    };
    #[inline]
    fn weight(&self) -> usize {
        self.as_ref().map_or(0, |some| {
            // SAFETY: Any memory location can fit in a `usize`, and
            // `Weight` measures only size that takes up memory,
            // so since the original value was representable in memory,
            // its weight will fit in a `usize`.
            unsafe { some.weight().unchecked_add(1) }
        })
    }
}

impl<T: Size> Size for Option<T> {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> = match T::MAX_SIZE {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(0)))
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite)) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite.plus(1)))
        }
    };
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        self.as_ref()
            .map_or(MaybeOverflow::Contained(0), |some| some.size().plus(1))
    }
}

impl<T: Corner> Corner for Option<T> {
    type Corners = iter::Chain<iter::Once<Self>, iter::Map<T::Corners, fn(T) -> Self>>;
    #[inline]
    fn corners() -> Self::Corners {
        iter::once(None).chain(T::corners().map(
            #[expect(
                clippy::as_conversions,
                reason = "Function pointer conversions are checked more thoroughly"
            )]
            {
                Some as fn(_) -> _
            },
        ))
    }
}

impl<T: Rnd> Rnd for Option<T> {
    #[inline]
    fn rnd<Rng: rand_core::RngCore>(
        rng: &mut Rng,
        expected_weight: f32,
    ) -> MaybeInstantiable<Self> {
        #[expect(clippy::modulo_arithmetic, reason = "intentional")]
        let variant_selector = {
            let rnd = f32::from_bits(rng.next_u32());
            rnd % (expected_weight + 1.)
        };
        MaybeInstantiable::Instantiable(
            if variant_selector >= 1.
                && let MaybeInstantiable::Instantiable(some) = T::rnd(rng, expected_weight)
            {
                Some(some)
            } else {
                None
            },
        )
    }
}

impl<T: Decimate> Decimate for Option<T> {
    type Decimate = Either<
        iter::Flatten<option::IntoIter<iter::Map<T::Decimate, fn(T) -> Self>>>,
        iter::Once<Self>,
    >;
    #[inline]
    fn decimate(&self, weight: usize) -> Self::Decimate {
        weight
            .checked_sub(1)
            .map_or_else(
                || Either::B(iter::once(None)),
                |weight| {
                    Either::A(
                        self.as_ref()
                            .map(|some| {
                                some.decimate(weight).map(
                                    #[expect(
                                        clippy::as_conversions,
                                        reason = "Function pointer conversions are checked more thoroughly"
                                    )]
                                    {
                                        Some as fn(_) -> _
                                    },
                                )
                            })
                            .into_iter()
                            .flatten(),
                    )
                },
            )
    }
}

impl<T: Refine> Refine for Option<T> {
    type Refine = Either<iter::Map<T::Refine, fn(T) -> Self>, option::IntoIter<Self>>;
    #[inline]
    fn refine(&self, size: usize) -> Self::Refine {
        self.as_ref().map_or_else(
            || Either::B(Some(None).into_iter()),
            |some| {
                size.checked_sub(1).map_or_else(
                    || Either::B(None.into_iter()),
                    |size| {
                        Either::A(some.refine(size).map(
                            #[expect(
                                clippy::as_conversions,
                                reason = "Function pointer conversions are checked more thoroughly"
                            )]
                            {
                                Some as fn(_) -> _
                            },
                        ))
                    },
                )
            },
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;

    crate::impl_tests!(Infallible, void);
    crate::impl_tests!((), unit);
    crate::impl_tests!(bool, bool);
    crate::impl_tests!(Option<Infallible>, option_void);
    crate::impl_tests!(Option<()>, option_unit);
    crate::impl_tests!(Option<bool>, option_bool); // TODO: remove and switch to the below
    // crate::impl_tests!(Option<u8>, option_u8); // TODO
}
