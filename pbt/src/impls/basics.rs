use {
    crate::{
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{
            corner::Corner, decimate::Decimate, refine::Refine, rnd::Rnd, size::Size,
            weight::Weight,
        },
    },
    core::{convert::Infallible, iter},
    std::hint::unreachable_unchecked,
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
    type Decimate = iter::Once<Self>;
    #[inline]
    fn decimate(&self) -> Self::Decimate {
        iter::once(())
    }
}

impl Refine for () {
    type Refine = iter::Once<Self>;
    #[inline]
    fn refine(&self) -> Self::Refine {
        iter::once(())
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
    fn decimate(&self) -> Self::Decimate {
        iter::empty()
    }
}

impl Refine for Infallible {
    type Refine = iter::Empty<Self>;
    #[inline]
    fn refine(&self) -> Self::Refine {
        iter::empty()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    crate::impl_tests!((), unit);
    crate::impl_tests!(Infallible, void);
}
