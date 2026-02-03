//! Implementations for vectors (`Vec<_>`).

use {
    crate::{
        conjure::{Conjure, ConjureAsync, Seed},
        count::{Cardinality, Count},
        shrink::Shrink,
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

impl<T: Shrink> Shrink for Vec<T> {
    #[inline]
    fn step<P: for<'s> FnMut(&'s Self) -> bool>(&self, property: &mut P) -> Option<Self> {
        // TODO: this seems wildly inefficient with all the copying,
        // but is there a general solution that will not interfere with other types?
        // maybe let `P` take `<T as Deref>::Target` or something like that

        // Find the minimal contiguous slice by
        // adjusting left and right bounds
        // with binary search:
        let mut acc: Self = if let Some(mut jump) =
            const { isize::MIN.cast_unsigned() }.checked_shr(self.len().leading_zeros())
        {
            let mut lhs = 0_usize;
            let mut rhs = jump.checked_shl(1).unwrap_or(usize::MAX);
            while jump != 0 {
                // Advance `lhs` temporarily:
                {
                    // SAFETY: `jump` starts at half this type's bit width (at most)
                    // and then successively cuts itself in half;
                    // that series (1/2 + 1/4 + 1/8 + ...) approaches but never reaches 1
                    let tmp_lhs = unsafe { lhs.unchecked_add(jump) };
                    if tmp_lhs <= self.len()
                        && property(
                            // SAFETY: `tmp_lhs <= self.len()` prevents `lhs > self.len()` and
                            // `rhs.min(self.len())` directly secures the other bound.
                            &unsafe { self.get_unchecked(tmp_lhs..rhs.min(self.len())) }.to_vec(),
                        )
                    {
                        lhs = tmp_lhs;
                    }
                }

                // Advance `rhs` temporarily:
                {
                    // SAFETY: `jump` starts at half this type's bit width (at most)
                    // and then successively cuts itself in half;
                    // that series (1/2 + 1/4 + 1/8 + ...) approaches but never reaches 1
                    let tmp_rhs = unsafe { rhs.unchecked_sub(jump) };
                    if tmp_rhs >= self.len()
                        || property(
                            // SAFETY: `tmp_lhs <= self.len()` prevents `lhs > self.len()` and
                            // `rhs.min(self.len())` directly secures the other bound.
                            &unsafe { self.get_unchecked(lhs..tmp_rhs.min(self.len())) }.to_vec(),
                        )
                    {
                        rhs = tmp_rhs;

                        // TODO: retry advancing `lhs` again?
                    }
                }

                jump >>= 1_u8;
            }

            // SAFETY: `tmp_lhs <= self.len()` prevents `lhs > self.len()` and
            // `rhs.min(self.len())` directly secures the other bound.
            unsafe { self.get_unchecked(lhs..rhs.min(self.len())) }.to_vec()
        } else {
            vec![]
        };
        if acc.len() < self.len() {
            return Some(acc);
        }

        // Remove elements within the contiguous slice:
        let mut i = 1;
        'remove: while i < acc.len() {
            let removed = {
                let mut r = acc.clone();
                let _: T = r.remove(i);
                r
            };
            if property(&removed) {
                acc = removed;
                // i = 0; // TODO: worth it?
            } else {
                let Some(incremented) = i.checked_add(1) else {
                    break 'remove;
                };
                i = incremented;
            }
        }
        if acc.len() < self.len() {
            return Some(acc);
        }

        // Shrink each element individually:
        let mut any = false;
        for i in 0..acc.len() {
            // SAFETY: `acc.len()` does not change, and `i` does not exceed it
            let element = unsafe { acc.get_unchecked(i) };
            if let Some(shrunk) = element.step(&mut |shrunk: &T| {
                let mut acc = acc.clone();
                // SAFETY: `acc.len()` does not change, and `i` does not exceed it
                *unsafe { acc.get_unchecked_mut(i) } = shrunk.clone();
                property(&acc)
            }) {
                any = true;
                // SAFETY: `acc.len()` does not change, and `i` does not exceed it
                *unsafe { acc.get_unchecked_mut(i) } = shrunk;
            }
        }
        any.then_some(acc)
    }
}
