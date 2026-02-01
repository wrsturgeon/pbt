//! Implementations for results (`Result<_, _>`).

use crate::{
    conjure::{Conjure, ConjureAsync, Seed},
    count::{Cardinality, Count},
    decompose::{Decompose, Decomposition},
};

impl<T: Count, E: Count> Count for Result<T, E> {
    const CARDINALITY: Cardinality = T::CARDINALITY.sum(E::CARDINALITY);
}

impl<T: Conjure, E: Conjure> Conjure for Result<T, E> {
    #[inline]
    fn conjure(mut seed: Seed, size: usize) -> Option<Self> {
        match const { (T::CARDINALITY, E::CARDINALITY) } {
            (Cardinality::Empty, Cardinality::Empty) => None,
            (Cardinality::Empty, _) => Some(Err(E::conjure(seed, size)?)),
            (_, Cardinality::Empty) => Some(Ok(T::conjure(seed, size)?)),
            (_, _) => Some({
                if seed.prng_bool() {
                    Err(E::conjure(seed, size)?)
                } else {
                    Ok(T::conjure(seed, size)?)
                }
            }),
        }
    }

    #[inline]
    fn corners() -> impl Iterator<Item = Self> {
        T::corners().map(Ok).chain(E::corners().map(Err))
    }

    #[inline]
    fn leaf(mut seed: Seed) -> Option<Self> {
        match const { (T::CARDINALITY, E::CARDINALITY) } {
            (Cardinality::Empty, Cardinality::Empty) => None,
            (Cardinality::Empty, _) => Some(Err(E::leaf(seed)?)),
            (_, Cardinality::Empty) => Some(Ok(T::leaf(seed)?)),
            (_, _) => Some({
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
    async fn conjure_async(mut seed: Seed, size: usize) -> Option<Self> {
        match const { (T::CARDINALITY, E::CARDINALITY) } {
            (Cardinality::Empty, Cardinality::Empty) => None,
            (Cardinality::Empty, _) => Some(Err(E::conjure_async(seed, size).await?)),
            (_, Cardinality::Empty) => Some(Ok(T::conjure_async(seed, size).await?)),
            (_, _) => Some({
                if seed.prng_bool() {
                    Err(E::conjure_async(seed, size).await?)
                } else {
                    Ok(T::conjure_async(seed, size).await?)
                }
            }),
        }
    }
}

impl<T: Decompose, E: Decompose> Decompose for Result<T, E> {
    #[inline]
    fn decompose(&self) -> Decomposition {
        Decomposition(match *self {
            Ok(ref ok) => vec![false.decompose(), ok.decompose()],
            Err(ref err) => vec![true.decompose(), err.decompose()],
        })
    }

    #[inline]
    fn from_decomposition(d: &Decomposition) -> Option<Self> {
        let (err, etc) = Decompose::from_decomposition(d)?;
        if err {
            E::from_decomposition(&etc).map(Err)
        } else {
            T::from_decomposition(&etc).map(Ok)
        }
    }
}

#[cfg(test)]
mod test {
    use crate::decompose;

    #[test]
    fn decomposition_roundtrip() {
        let () = decompose::check_roundtrip::<Result<Vec<u8>, Vec<u8>>>();
    }
}
