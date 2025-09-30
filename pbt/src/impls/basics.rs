use {
    crate::{
        either::Either,
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{
            corner::Corner, decimate::Decimate, refine::Refine, rnd::Rnd, size::Size,
            weight::Weight,
        },
    },
    core::{convert::Infallible, hint::unreachable_unchecked, iter, option},
};

impl Size for () {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(0)));
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(0)
    }
}

impl Weight for () {
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> =
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0));
    #[inline]
    fn weight(&self) -> usize {
        0
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
        _expected_weight: usize,
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

impl Size for Infallible {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> =
        MaybeInstantiable::Uninstantiable;
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        // SAFETY: Uninstantiable type.
        unsafe { unreachable_unchecked() }
    }
}

impl Weight for Infallible {
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> = MaybeInstantiable::Uninstantiable;
    #[inline]
    fn weight(&self) -> usize {
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
        _expected_weight: usize,
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

impl<T: Weight> Weight for Option<T> {
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
            // `Weight` measures only size that takes up memory.
            unsafe { some.weight().unchecked_add(1) }
        })
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
        expected_weight: usize,
    ) -> MaybeInstantiable<Self> {
        MaybeInstantiable::Instantiable(
            if let Some(expected_weight) = expected_weight.checked_sub(1)
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

    crate::impl_tests!((), unit);
    crate::impl_tests!(Infallible, void);
    crate::impl_tests!(Option<Infallible>, option_void);
    crate::impl_tests!(Option<()>, option_unit);
}
