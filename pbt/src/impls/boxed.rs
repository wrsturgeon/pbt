//! Implementations for boxed values (`Box<_>`).

use {
    crate::{
        conjure::{Conjure, ConjureAsync, Seed, Uninstantiable},
        count::{Cardinality, Count},
        shrink::Shrink,
    },
    core::iter,
};

impl<T: Count> Count for Box<T> {
    const CARDINALITY: Cardinality = T::CARDINALITY;
}

impl<T: Conjure> Conjure for Box<T> {
    #[inline]
    fn conjure(seed: Seed) -> Result<Self, Uninstantiable> {
        Ok(Box::new(T::conjure(seed)?))
    }

    #[inline]
    fn corners() -> Box<dyn Iterator<Item = Self>> {
        Box::new(T::corners().map(Box::new))
    }

    #[inline]
    fn variants() -> impl Iterator<Item = (Cardinality, fn(Seed) -> Self)> {
        iter::once((
            Self::CARDINALITY,
            (|seed| unsafe { Self::conjure(seed).unwrap_unchecked() }) as fn(_) -> _,
        ))
    }

    #[inline]
    fn leaf(seed: Seed) -> Result<Self, Uninstantiable> {
        Ok(Box::new(T::leaf(seed)?))
    }
}

impl<T: ConjureAsync> ConjureAsync for Box<T> {
    #[inline]
    async fn conjure_async(seed: Seed) -> Result<Self, Uninstantiable> {
        T::conjure_async(seed).await.map(Self::new)
    }
}

impl<T: Shrink> Shrink for Box<T> {
    #[inline]
    fn step<P: for<'s> FnMut(&'s Self) -> bool + ?Sized>(&self, property: &mut P) -> Option<Self> {
        let closure: &mut dyn FnMut(&T) -> bool = &mut |t| property(&Self::new(t.clone()));
        T::step(&**self, closure).map(Self::new)
    }
}
