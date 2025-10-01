use {
    crate::{
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{
            corner::Corner, decimate::Decimate, refine::Refine, rnd::Rnd, size::Size,
            weight::Weight,
        },
    },
    core::{array, convert::Infallible, hint::unreachable_unchecked, iter, option},
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
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(1)));
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
    type Decimate = option::IntoIter<Self>;
    #[inline]
    fn decimate(&self, weight: usize) -> Self::Decimate {
        (weight == 0).then_some(*self).into_iter()
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

#[cfg(test)]
mod test {
    use super::*;

    crate::impl_tests!(Infallible, void);
    crate::impl_tests!((), unit);
    crate::impl_tests!(bool, bool);
}
