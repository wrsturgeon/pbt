use {
    crate::{
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{corner::Corner, decimate::Decimate, refine::Refine, size::Size, weight::Weight},
    },
    core::iter,
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

#[cfg(test)]
mod test {
    use super::*;

    crate::impl_tests!((), unit);
}
