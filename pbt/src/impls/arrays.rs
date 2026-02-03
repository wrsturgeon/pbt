//! Implementations for arrays.

#![expect(clippy::multiple_unsafe_ops_per_block, reason = "related")]

use {
    crate::{
        conjure::{Conjure, Seed},
        count::{Cardinality, Count},
        shrink::Shrink,
    },
    core::{iter, mem::MaybeUninit},
};

impl<T: Count, const N: usize> Count for [T; N] {
    // NOTE: Not like vectors, since `[!; 10]` can't be `[]`.
    const CARDINALITY: Cardinality = T::CARDINALITY;
}

impl<T: Conjure, const N: usize> Conjure for [T; N] {
    #[inline]
    fn conjure(mut seed: Seed, size: usize) -> Option<Self> {
        let seeds = seed.partition::<N>(size);

        let mut acc = const { MaybeUninit::<[T; N]>::uninit() };
        for (i, (seed, size)) in seeds.into_iter().enumerate() {
            // SAFETY: In bounds and types match.
            let uninit = unsafe { &mut *acc.as_mut_ptr().cast::<MaybeUninit<T>>().add(i) };
            let _: &mut _ = uninit.write(T::conjure(seed, size)?);
        }
        // SAFETY: Iterated over all `N` elements above.
        let acc = unsafe { acc.assume_init() };
        Some(acc)
    }

    #[inline]
    fn corners() -> impl Iterator<Item = Self> {
        // TODO: proper implementation (e.g. [A, ..., A], [A, ..., B], ... [B, ..., A], ...)
        iter::empty()
    }

    #[inline]
    fn leaf(mut seed: Seed) -> Option<Self> {
        let mut acc = const { MaybeUninit::<[T; N]>::uninit() };
        for i in const { 0..N } {
            // SAFETY: In bounds and types match.
            let uninit = unsafe { &mut *acc.as_mut_ptr().cast::<MaybeUninit<T>>().add(i) };
            let _: &mut _ = uninit.write(T::leaf(seed.split())?);
        }
        // SAFETY: Iterated over all `N` elements above.
        let acc = unsafe { acc.assume_init() };
        Some(acc)
    }
}

impl<T: Shrink, const N: usize> Shrink for [T; N] {
    #[inline]
    fn step<P: for<'s> FnMut(&'s Self) -> bool>(&self, property: &mut P) -> Option<Self> {
        let mut acc = self.clone();
        let mut any = false;
        for i in const { 0..N } {
            // SAFETY: `i` (defined above) cannot exceed `N`
            if let Some(reduced) = unsafe { acc.get_unchecked(i) }.step(&mut |t: &T| {
                let mut acc = acc.clone();
                // SAFETY: `i` (defined above) cannot exceed `N`
                *unsafe { acc.get_unchecked_mut(i) } = t.clone();
                property(&acc)
            }) {
                // SAFETY: `i` (defined above) cannot exceed `N`
                *unsafe { acc.get_unchecked_mut(i) } = reduced;
                any = true;
            }
        }
        any.then_some(acc)
    }
}
