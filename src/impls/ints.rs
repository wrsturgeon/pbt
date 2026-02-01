//! Implementations for standard fixed-bit-width integral types (e.g. `u8`) and `bool`.

use crate::{
    conjure::{Conjure, ConjureAsync, Seed},
    count::{Cardinality, Count},
};

/// Implement `Count` and `Conjure` for integral types of a given
/// bit width less than or equal to 64.
macro_rules! impl_le_64b {
    ($i:ident, $u:ident) => {
        impl Count for $i {
            const CARDINALITY: Cardinality = Cardinality::Finite;
        }

        impl Conjure for $i {
            #[inline(always)]
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

            #[inline(always)]
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
            #[inline(always)]
            async fn conjure_async(seed: Seed, _size: usize) -> Option<Self> {
                Self::leaf(seed)
            }
        }

        impl Count for $u {
            const CARDINALITY: Cardinality = Cardinality::Finite;
        }

        impl Conjure for $u {
            #[inline(always)]
            fn conjure(seed: Seed, size: usize) -> Option<Self> {
                Some($i::conjure(seed, size)?.cast_unsigned())
            }

            #[inline(always)]
            fn corners() -> impl Iterator<Item = Self> {
                $i::corners().map($i::cast_unsigned)
            }

            #[inline(always)]
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
            #[inline(always)]
            async fn conjure_async(seed: Seed, size: usize) -> Option<Self> {
                Some($i::conjure_async(seed, size).await?.cast_unsigned())
            }
        }
    };
}

impl_le_64b!(i8, u8);
impl_le_64b!(i16, u16);
impl_le_64b!(i32, u32);
impl_le_64b!(i64, u64);

impl Count for bool {
    const CARDINALITY: Cardinality = Cardinality::Finite;
}

impl Conjure for bool {
    #[inline(always)]
    fn conjure(mut seed: Seed, _size: usize) -> Option<Self> {
        Some(seed.prng_bool())
    }

    #[inline(always)]
    fn corners() -> impl Iterator<Item = Self> {
        [false, true].into_iter()
    }

    #[inline(always)]
    fn leaf(mut seed: Seed) -> Option<Self> {
        Some(seed.prng_bool())
    }
}

impl ConjureAsync for bool {
    #[inline(always)]
    async fn conjure_async(mut seed: Seed, _size: usize) -> Option<Self> {
        Some(seed.prng_bool())
    }
}
