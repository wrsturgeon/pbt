//! Implementations for standard fixed-bit-width integral types (e.g. `u8`) and `bool`.

use crate::{
    conjure::{Conjure, ConjureAsync, Seed},
    count::{Cardinality, Count},
    shrink::Shrink,
};

/// Implement `Count` and `Conjure` for integral types of a given
/// bit width less than or equal to 64.
macro_rules! impl_le_64b {
    ($i:ident, $u:ident) => {
        impl Count for $i {
            const CARDINALITY: Cardinality = Cardinality::Finite;
        }

        impl Conjure for $i {
            #[inline]
            fn conjure(seed: Seed, _size: usize) -> Option<Self> {
                Self::leaf(seed)
            }

            #[inline]
            fn corners() -> impl Iterator<Item = Self> {
                [
                    0,
                    1, // ?
                    Self::MAX,
                    Self::MIN,
                    -1,
                ]
                .into_iter()
            }

            #[inline]
            #[allow(
                clippy::allow_attributes,
                clippy::as_conversions,
                clippy::cast_possible_truncation,
                clippy::cast_possible_wrap,
                reason = "intentional"
            )]
            fn leaf(mut seed: Seed) -> Option<Self> {
                Some(seed.prng() as Self)
            }
        }

        impl ConjureAsync for $i {
            #[inline]
            async fn conjure_async(seed: Seed, _size: usize) -> Option<Self> {
                Self::leaf(seed)
            }
        }

        impl Shrink for $i {
            #[inline]
            fn step<P: for<'s> FnMut(&'s Self) -> bool>(&self, property: &mut P) -> Option<Self> {
                const ZERO: $i = 0;

                #[inline]
                #[expect(clippy::single_call_fn, reason = "complement to the below")]
                fn decompose(i: $i) -> (bool, $u) {
                    (i < 0, i.unsigned_abs())
                }

                #[inline]
                fn recompose(neg: bool, abs: $u) -> Option<$i> {
                    let i = $i::try_from(abs).ok()?;
                    if neg {
                        i.checked_neg()
                    } else {
                        Some(i)
                    }
                }

                if *self < ZERO
                    && let Some(pos) = self.checked_neg()
                    && property(&pos)
                {
                    return Some(pos);
                }

                let (neg, abs) = <(bool, $u) as Shrink>::step(&decompose(*self), &mut |&(neg, abs)| {
                    if let Some(recomposed) = recompose(neg, abs) {
                        property(&recomposed)
                    } else {
                        false
                    }
                })?;
                recompose(neg, abs)
            }
        }

        impl Count for $u {
            const CARDINALITY: Cardinality = Cardinality::Finite;
        }

        impl Conjure for $u {
            #[inline]
            fn conjure(seed: Seed, size: usize) -> Option<Self> {
                Some($i::conjure(seed, size)?.cast_unsigned())
            }

            #[inline]
            fn corners() -> impl Iterator<Item = Self> {
                $i::corners().map($i::cast_unsigned)
            }

            #[inline]
            #[allow(
                clippy::allow_attributes,
                clippy::as_conversions,
                clippy::cast_possible_truncation,
                reason = "intentional"
            )]
            fn leaf(seed: Seed) -> Option<Self> {
                Some($i::leaf(seed)?.cast_unsigned())
            }
        }

        impl ConjureAsync for $u {
            #[inline]
            async fn conjure_async(seed: Seed, size: usize) -> Option<Self> {
                Some($i::conjure_async(seed, size).await?.cast_unsigned())
            }
        }

        impl Shrink for $u {
            #[inline]
            fn step<P: for<'s> FnMut(&'s Self) -> bool>(&self, property: &mut P) -> Option<Self> {
                // Take large (but successively smaller) steps first:
                let mut shift = 0;
                'logarithmic: loop {
                    let Some(subtrahend) = self.checked_shr(shift) else {
                        break 'logarithmic;
                    };
                    if subtrahend == 0 {
                        break 'logarithmic;
                    }
                    let difference = self.checked_sub(subtrahend)?;
                    if property(&difference) {
                        return Some(difference);
                    }
                    let Some(next_shift) = shift.checked_add(1) else {
                        break 'logarithmic;
                    };
                    shift = next_shift;
                }

                /*
                // If none of those succeeded, fill in the gaps:
                let mut fuel = u8::MAX;
                'linear: while let Some(next_shift) = shift.checked_sub(1) {
                    let lhs = self.checked_shr(shift);
                    shift = next_shift;
                    let Some(lhs) = lhs.and_then(|u| u.checked_add(1)) else {
                        continue 'linear;
                    };
                    let Some(rhs) = self.checked_shr(shift) else {
                        continue 'linear;
                    };
                    for u in lhs..rhs {
                        fuel = fuel.checked_sub(1)?;
                        if property(&u) {
                            return Some(u);
                        }
                    }
                }
                */

                None
            }
        }

        #[cfg(test)]
        mod $u {
            use super::*;

            #[test]
            fn shrink() {
                const N_TRIALS: usize = 1_000;

                let mut seed = Seed::new();
                for size in 0..N_TRIALS {
                    println!();

                    let (mut minimal, mut greater) = <($u, $u)>::conjure(seed.split(), size).unwrap();
                    if minimal > greater {
                        let () = ::core::mem::swap(&mut minimal, &mut greater);
                    }
                    let shrunk = crate::shrink::minimal(&greater, |&i: &$u| {
                        print!("{i:?}");
                        let greater = i >= minimal;
                        println!(" {}", if greater { 'Y' } else { 'N' });
                        greater
                    });
                    pretty_assertions::assert_eq!(
                        minimal,
                        shrunk,
                        "{greater:?} shrunk to {shrunk:?}, but it should have shrunk further to {minimal:?}",
                    );
                }
            }
        }
    };
}

impl Count for bool {
    const CARDINALITY: Cardinality = Cardinality::Finite;
}

impl Conjure for bool {
    #[inline]
    fn conjure(mut seed: Seed, _size: usize) -> Option<Self> {
        Some(seed.prng_bool())
    }

    #[inline]
    fn corners() -> impl Iterator<Item = Self> {
        [false, true].into_iter()
    }

    #[inline]
    fn leaf(mut seed: Seed) -> Option<Self> {
        Some(seed.prng_bool())
    }
}

impl ConjureAsync for bool {
    #[inline]
    async fn conjure_async(mut seed: Seed, _size: usize) -> Option<Self> {
        Some(seed.prng_bool())
    }
}

impl Shrink for bool {
    #[inline]
    fn step<P: for<'s> FnMut(&'s Self) -> bool>(&self, property: &mut P) -> Option<Self> {
        (*self && property(&false)).then_some(false)
    }
}

impl_le_64b!(i8, u8);
impl_le_64b!(i16, u16);
impl_le_64b!(i32, u32);
impl_le_64b!(i64, u64);

// this is intentionally a counterexample; shrinking is not perfect:
/*
#[cfg(test)]
mod test {

    #[test]
    fn shrink_mod_100() {
        let orig = 300_u16;
        let shrunk = crate::shrink::minimal(&orig, |&u| {
            print!("{u}");
            let success = u > 0 && u % 100 == 0;
            println!(" {}", if success { 'Y' } else { 'N' });
            success
        });
        pretty_assertions::assert_eq!(100, shrunk);
    }
}
*/
