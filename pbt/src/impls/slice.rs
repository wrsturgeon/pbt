//! Implementations for `&[_]`.

use crate::{
    size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
    traits::{size::Size, weight::Weight},
};

impl<T: Weight> Weight for [T] {
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> = match T::MAX_EXPECTED_WEIGHT
    {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0.))
        }
        MaybeInstantiable::Instantiable(_) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
    };
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> = match T::MAX_WEIGHT {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0))
        }
        MaybeInstantiable::Instantiable(_) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
    };
    #[inline]
    fn weight(&self) -> usize {
        let mut acc: usize = 0;
        #[expect(clippy::multiple_unsafe_ops_per_block, reason = "Logically connected.")]
        for element in self {
            // SAFETY: Any memory location can fit in a `usize`, and
            // `Weight` measures only size that takes up memory,
            // so since the original value was representable in memory,
            // its weight will fit in a `usize`.
            acc = unsafe { acc.unchecked_add(1).unchecked_add(element.weight()) };
        }
        acc
    }
}

impl<T: Size> Size for [T] {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> = match T::MAX_SIZE {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(0)))
        }
        MaybeInstantiable::Instantiable(_) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
    };
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        let mut acc: usize = 0;
        for element in self {
            let Some(incremented) = acc.checked_add(1) else {
                return MaybeOverflow::Overflow;
            };
            let MaybeOverflow::Contained(marginal) = element.size() else {
                return MaybeOverflow::Overflow;
            };
            let Some(increased) = incremented.checked_add(marginal) else {
                return MaybeOverflow::Overflow;
            };
            acc = increased;
        }
        MaybeOverflow::Contained(acc)
    }
}
