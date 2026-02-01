//! Implementations for vectors (`Vec<_>`).

use {
    crate::{
        conjure::{Conjure, ConjureAsync, Seed},
        count::{Cardinality, Count},
        decompose::{Decompose, Decomposition},
    },
    core::iter,
    futures::{StreamExt as _, stream::FuturesOrdered},
};

impl<T: Count> Count for Vec<T> {
    const CARDINALITY: Cardinality = match T::CARDINALITY {
        Cardinality::Empty => {
            // NOTE: THIS IS COUNTERINTUITIVE!
            // A `Vec<!>`, for example, can only be `vec![]`.
            Cardinality::Finite
        }
        Cardinality::Finite | Cardinality::Infinite => Cardinality::Infinite,
    };
}

impl<T: Conjure> Conjure for Vec<T> {
    #[inline]
    fn conjure(mut seed: Seed, mut size: usize) -> Option<Self> {
        Some(match T::CARDINALITY {
            Cardinality::Empty => vec![],
            Cardinality::Finite | Cardinality::Infinite => {
                let mut acc = vec![];
                while let Some([(head_seed, head_size), (tail_seed, tail_size)]) =
                    seed.should_recurse(size)
                {
                    let () = acc.push(T::conjure(head_seed, head_size)?);
                    seed = tail_seed;
                    size = tail_size;
                }
                acc
            }
        })
    }

    #[inline]
    fn corners() -> impl Iterator<Item = Self> {
        iter::once(vec![]).chain(T::corners().map(|singleton| vec![singleton]))
    }

    #[inline]
    fn leaf(_seed: Seed) -> Option<Self> {
        Some(vec![])
    }
}

impl<T: ConjureAsync> ConjureAsync for Vec<T> {
    #[inline]
    async fn conjure_async(mut seed: Seed, mut size: usize) -> Option<Self> {
        Some(match T::CARDINALITY {
            Cardinality::Empty => vec![],
            Cardinality::Finite | Cardinality::Infinite => {
                let mut acc = FuturesOrdered::new();
                while let Some([(head_seed, head_size), (tail_seed, tail_size)]) =
                    seed.should_recurse(size)
                {
                    let () = acc.push_back(async move {
                        let opt = T::conjure_async(head_seed, head_size).await;
                        // SAFETY: `T` verified not to be empty above.
                        unsafe { opt.unwrap_unchecked() }
                    });
                    seed = tail_seed;
                    size = tail_size;
                }
                acc.collect().await
            }
        })
    }
}

impl<T: Decompose> Decompose for Vec<T> {
    #[inline]
    fn decompose(&self) -> Decomposition {
        Decomposition(self.iter().map(T::decompose).collect())
    }

    #[inline]
    fn from_decomposition(d: &Decomposition) -> Option<Self> {
        d.0.iter().map(T::from_decomposition).collect()
    }
}

#[cfg(test)]
mod test {
    use crate::decompose;

    #[test]
    fn decomposition_roundtrip() {
        let () = decompose::check_roundtrip::<Vec<Vec<u8>>>();
    }
}
