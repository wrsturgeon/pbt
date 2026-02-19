//! Implementations for results (`Result<_, _>`).

use crate::{
    conjure::{Conjure, ConjureAsync, Seed, Uninstantiable},
    count::{Cardinality, Count},
    shrink::Shrink,
};

impl<T: Count, E: Count> Count for Result<T, E> {
    const CARDINALITY: Cardinality = T::CARDINALITY.of_sum(E::CARDINALITY);
}

impl<T: Conjure, E: Conjure> Conjure for Result<T, E> {
    #[inline]
    fn conjure(mut seed: Seed) -> Result<Self, Uninstantiable> {
        match const { (T::CARDINALITY, E::CARDINALITY) } {
            (Cardinality::Empty, Cardinality::Empty) => Err(Uninstantiable),
            (Cardinality::Empty, _) => Ok(Err(E::conjure(seed)?)),
            (_, Cardinality::Empty) => Ok(Ok(T::conjure(seed)?)),
            (_, _) => Ok({
                if seed.prng_bool() {
                    Err(E::conjure(seed)?)
                } else {
                    Ok(T::conjure(seed)?)
                }
            }),
        }
    }

    #[inline]
    fn corners() -> Box<dyn Iterator<Item = Self>> {
        Box::new(T::corners().map(Ok).chain(E::corners().map(Err)))
    }

    #[inline]
    fn variants() -> impl Iterator<Item = (Cardinality, fn(Seed) -> Self)> {
        [
            (
                T::CARDINALITY,
                (|seed| Ok(unsafe { T::conjure(seed).unwrap_unchecked() })) as fn(_) -> _,
            ),
            (
                E::CARDINALITY,
                (|seed| Err(unsafe { E::conjure(seed).unwrap_unchecked() })) as fn(_) -> _,
            ),
        ]
        .into_iter()
    }

    #[inline]
    fn leaf(mut seed: Seed) -> Result<Self, Uninstantiable> {
        match const { (T::CARDINALITY, E::CARDINALITY) } {
            (Cardinality::Empty, Cardinality::Empty) => Err(Uninstantiable),
            (Cardinality::Empty, _) => Ok(Err(E::leaf(seed)?)),
            (_, Cardinality::Empty) => Ok(Ok(T::leaf(seed)?)),
            (_, _) => Ok({
                if seed.prng_bool() {
                    Err(E::leaf(seed)?)
                } else {
                    Ok(T::leaf(seed)?)
                }
            }),
        }
    }
}

impl<T: ConjureAsync, E: ConjureAsync> ConjureAsync for Result<T, E> {
    #[inline]
    async fn conjure_async(mut seed: Seed) -> Result<Self, Uninstantiable> {
        match const { (T::CARDINALITY, E::CARDINALITY) } {
            (Cardinality::Empty, Cardinality::Empty) => Err(Uninstantiable),
            (Cardinality::Empty, _) => Ok(Err(E::conjure_async(seed).await?)),
            (_, Cardinality::Empty) => Ok(Ok(T::conjure_async(seed).await?)),
            (_, _) => Ok({
                if seed.prng_bool() {
                    Err(E::conjure_async(seed).await?)
                } else {
                    Ok(T::conjure_async(seed).await?)
                }
            }),
        }
    }
}

impl<T: Shrink, E: Shrink> Shrink for Result<T, E> {
    #[inline]
    fn step<P: for<'s> FnMut(&'s Self) -> bool + ?Sized>(&self, property: &mut P) -> Option<Self> {
        match *self {
            Ok(ref ok) => ok.step(&mut |ok| property(&Ok(ok.clone()))).map(Ok),
            // TODO: corners of `Ok` as well?
            Err(ref err) => err.step(&mut |err| property(&Err(err.clone()))).map(Err),
        }
    }
}
