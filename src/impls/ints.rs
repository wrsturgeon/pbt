//! Implementations for standard fixed-bit-width integral types (e.g. `u8`) and `bool`.

use {
    crate::{
        conjure::{Conjure, ConjureAsync, Seed},
        count::{Cardinality, Count},
        decompose::{Decompose, Decomposition},
    },
    core::array,
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

        impl Decompose for $i {
            #[inline]
            fn decompose(&self) -> Decomposition {
                ((*self < 0), self.unsigned_abs()).decompose()
            }

            #[inline]
            fn from_decomposition(d: &Decomposition) -> Option<Self> {
                let (negate, $u) = Decompose::from_decomposition(d)?;
                let $i = $u::cast_signed($u);
                Some(if negate && let Some(negative) = $i.checked_neg() {
                    negative
                } else {
                    $i
                })
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

        impl Decompose for $u {
            #[inline]
            fn decompose(&self) -> Decomposition {
                #[expect(
                    clippy::as_conversions,
                    reason = "If integer bit width exceeds `usize`, there are much bigger problems"
                )]
                <[bool; Self::BITS as usize]>::decompose(&array::from_fn(|i| {
                    (*self & (1 << i)) != 0
                }))
            }

            #[inline]
            fn from_decomposition(d: &Decomposition) -> Option<Self> {
                #[expect(
                    clippy::as_conversions,
                    reason = "If integer bit width exceeds `usize`, there are much bigger problems"
                )]
                let bits = <[bool; Self::BITS as usize]>::from_decomposition(d)?;
                let mut acc: Self = 0;
                for (i, bit) in bits.into_iter().enumerate() {
                    acc |= Self::from(bit) << i;
                }
                Some(acc)
            }
        }

        #[cfg(test)]
        mod $i {
            use crate::decompose;

            #[test]
            fn $i() {
                decompose::check_roundtrip::<$i>();
            }

            #[test]
            fn $u() {
                decompose::check_roundtrip::<$u>();
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

impl Decompose for bool {
    #[inline]
    fn decompose(&self) -> Decomposition {
        Decomposition(if *self {
            vec![Decomposition(vec![])]
        } else {
            vec![]
        })
    }

    #[inline]
    fn from_decomposition(d: &Decomposition) -> Option<Self> {
        Some(!d.0.is_empty())
    }
}

impl_le_64b!(i8, u8);
impl_le_64b!(i16, u16);
impl_le_64b!(i32, u32);
impl_le_64b!(i64, u64);

#[cfg(test)]
mod test {
    use crate::decompose;

    #[test]
    fn bool() {
        decompose::check_roundtrip::<bool>();
    }
}
