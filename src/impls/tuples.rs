//! Implementations for tuples.

use crate::{
    conjure::{Conjure, Seed},
    count::{Cardinality, Count},
    shrink::Shrink,
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

impl<A: Shrink, B: Shrink> Shrink for (A, B) {
    #[inline]
    fn step<P: FnMut(&Self) -> bool>(&self, property: &mut P) -> Option<Self> {
        let (ref a, ref b) = *self;
        let sa = a.step(&mut |a| property(&(a.clone(), b.clone())));
        let sb = b.step(&mut |b| property(&(a.clone(), b.clone())));
        if let Some(a) = sa {
            Some((a, sb.unwrap_or_else(|| b.clone())))
        } else {
            sb.map(|b| (a.clone(), b))
        }
    }
}
