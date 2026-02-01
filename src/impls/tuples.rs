//! Implementations for tuples.

use crate::{
    conjure::{Conjure, Seed},
    count::{Cardinality, Count},
    decompose::{Decompose, Decomposition},
};

impl<A: Count, B: Count> Count for (A, B) {
    const CARDINALITY: Cardinality = A::CARDINALITY.prod(B::CARDINALITY);
}

impl<A: Conjure, B: Conjure> Conjure for (A, B) {
    #[inline]
    fn conjure(mut seed: Seed, size: usize) -> Option<Self> {
        let [(a_seed, a_size), (b_seed, b_size)] = seed.partition(size);
        Some((A::conjure(a_seed, a_size)?, B::conjure(b_seed, b_size)?))
    }

    #[inline]
    fn corners() -> impl Iterator<Item = Self> {
        A::corners()
            .enumerate()
            .flat_map(|(i, _a)| B::corners().filter_map(move |b| Some((A::corners().nth(i)?, b))))
    }

    #[inline]
    fn leaf(mut seed: Seed) -> Option<Self> {
        Some((A::leaf(seed.split())?, B::leaf(seed)?))
    }
}

impl<A: Decompose, B: Decompose> Decompose for (A, B) {
    #[inline]
    fn decompose(&self) -> Decomposition {
        let (ref a, ref b) = *self;
        Decomposition(vec![a.decompose(), b.decompose()])
    }

    #[inline]
    fn from_decomposition(d: &Decomposition) -> Option<Self> {
        let [a, b] = Decompose::from_decomposition(d)?;
        Some((A::from_decomposition(&a)?, B::from_decomposition(&b)?))
    }
}

#[cfg(test)]
mod test {
    use crate::decompose;

    #[test]
    fn decomposition_roundtrip() {
        let () = decompose::check_roundtrip::<(Vec<u8>, Vec<u8>)>();
    }
}
