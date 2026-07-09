//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variants},
        registration::Registration,
    },
    core::iter,
    wyrand::WyRand,
};

/// Shrink an integer by repeatedly subtracting half the previous shrunk amount.
macro_rules! shrink {
    ($u:ty) => {
        |n: $u| {
            let mut shift = 0;
            Box::new(iter::from_fn(move || {
                let delta = n.checked_shr(shift)?;
                if delta == 0 {
                    return None;
                }
                shift = shift.checked_add(1)?;
                n.checked_sub(delta)
            }))
        }
    };
}

/// Generate small integers using a geometric-ish bit-by-bit distribution.
macro_rules! small {
    ($u:ty) => {
        |prng| {
            let mut coin = CoinFlips::new(prng);
            if coin.flip(prng) {
                return 0;
            }
            let mut acc: $u = 1;
            while coin.flip(prng) {
                acc = acc.wrapping_shl(1) | <$u>::from(coin.flip(prng));
            }
            acc
        }
    };
}

/// Implement `Pbt` for `u_` up to `u64`, above which we need another strategy.
macro_rules! impl_unsigned {
    ($u:ty) => {
        impl Pbt for $u {
            #[inline]
            fn construct<F>(
                Parts {
                    mut fields,
                    variant_index,
                }: Parts<F>,
            ) -> Self
            where
                F: Fields,
            {
                debug_assert_eq!(variant_index, None, "unsigned integers are literals");
                fields.field()
            }

            #[inline]
            fn deconstruct(self) -> Parts<Store> {
                let mut fields = Store::new();
                let () = fields.push(self);
                Parts {
                    fields,
                    variant_index: None,
                }
            }

            #[inline]
            fn register(_registration: &mut Registration<'_>) -> Variants<Self> {
                Variants::Literal {
                    deserialize: |json| {
                        let serde_json::Value::String(ref s) = *json else {
                            return None;
                        };
                        s.parse().ok()
                    },
                    generators: vec![
                        |prng| {
                            #[allow(
                                clippy::allow_attributes,
                                clippy::as_conversions,
                                clippy::cast_possible_truncation,
                                reason = "intentional: bit width checked above"
                            )]
                            (prng.rand() as Self)
                        },
                        small!($u),
                    ],
                    serialize: |&i| i.to_string().into(),
                    shrink: shrink!($u),
                }
            }
        }
    };
}

impl_unsigned!(u8);
impl_unsigned!(u16);
impl_unsigned!(u32);
impl_unsigned!(u64);

/// A sequence of coin flips, powered by a pseudorandom number generator.
struct CoinFlips {
    /// Cached bits from a full `u64` PRNG generation.
    bit_reservoir: u64,
    /// The number of remaining cached bits from a full `u64` PRNG generation.
    remaining_bits: u8,
}

impl CoinFlips {
    /// Flip a coin: sample `bool` with equal probability of `true` or `false`.
    #[inline]
    fn flip(&mut self, prng: &mut WyRand) -> bool {
        if let Some(decrement) = self.remaining_bits.checked_sub(1) {
            self.remaining_bits = decrement;
        } else {
            self.bit_reservoir = prng.rand();
            self.remaining_bits = 63;
        }
        let bit = (self.bit_reservoir & 1) != 0;
        self.bit_reservoir >>= 1_u8;
        bit
    }

    /// A sequence of coin flips, powered by a pseudorandom number generator.
    #[inline]
    fn new(prng: &mut WyRand) -> Self {
        Self {
            bit_reservoir: prng.rand(),
            remaining_bits: 64,
        }
    }
}

impl Pbt for usize {
    #[inline]
    fn construct<F>(
        Parts {
            mut fields,
            variant_index,
        }: Parts<F>,
    ) -> Self
    where
        F: Fields,
    {
        debug_assert_eq!(variant_index, None, "`usize` is a literal");
        fields.field()
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: None,
        }
    }

    #[inline]
    fn register(_registration: &mut Registration<'_>) -> Variants<Self> {
        Variants::Literal {
            deserialize: |json| {
                let serde_json::Value::String(ref s) = *json else {
                    return None;
                };
                s.parse().ok()
            },
            generators: vec![uniform, small!(usize)],
            serialize: |&i| i.to_string().into(),
            shrink: shrink!(usize),
        }
    }
}

#[cfg(feature = "num-bigint")]
impl Pbt for num_bigint::BigUint {
    #[inline]
    fn construct<F>(
        Parts {
            mut fields,
            variant_index,
        }: Parts<F>,
    ) -> Self
    where
        F: Fields,
    {
        debug_assert_eq!(variant_index, None, "`num_bigint::BigUint` is a literal");
        fields.field()
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: None,
        }
    }

    #[inline]
    #[expect(clippy::arithmetic_side_effects, reason = "not with `BigUint`")]
    fn register(_registration: &mut Registration<'_>) -> Variants<Self> {
        #[inline]
        fn small(prng: &mut WyRand) -> num_bigint::BigUint {
            let mut coin = CoinFlips::new(prng);
            if coin.flip(prng) {
                return num_bigint::BigUint::ZERO;
            }
            let mut acc = num_bigint::BigUint::ONE;
            while coin.flip(prng) {
                acc = (acc << 1_u8) | num_bigint::BigUint::from(coin.flip(prng));
            }
            acc
        }

        #[inline]
        fn shrink(n: num_bigint::BigUint) -> Box<dyn Iterator<Item = num_bigint::BigUint>> {
            let mut shift: usize = 0;
            Box::new(iter::from_fn(move || {
                use num_traits::{Zero as _, ops::checked::CheckedSub as _};
                let delta = &n >> shift;
                if delta.is_zero() {
                    return None;
                }
                shift += 1;
                n.checked_sub(&delta)
            }))
        }

        Variants::Literal {
            deserialize: |json| {
                let serde_json::Value::String(ref s) = *json else {
                    return None;
                };
                s.parse().ok()
            },
            generators: vec![small],
            serialize: |i| i.to_string().into(),
            shrink,
        }
    }
}

/// Generate integers uniformly over the target machine word.
#[inline]
fn uniform(prng: &mut WyRand) -> usize {
    if const { usize::BITS <= 64 } {
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "intentional: bit width checked above"
        )]
        (prng.rand() as usize)
    } else {
        let mut acc: usize = 0;
        let mut bits: u32 = 0;
        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "truncation is impossible: bit width checked above"
        )]
        while bits < usize::BITS {
            // SAFETY: Barring extraterrestrial technology...
            bits = unsafe { bits.unchecked_add(64) };
            acc = acc.wrapping_shl(64) | (prng.rand() as usize);
        }
        acc
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        super::*,
        crate::{
            arbitrary::arbitrary, check_eta_expansion, check_serialization, persist,
            reflection::register_globally,
        },
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic_u8() {
        let mut prng = WyRand::new(42);
        let generated: Vec<u8> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<u8> = vec![9, 6, 6, 230, 88, 168, 3, 0, 1, 0];
        assert_eq!(generated, expected);
    }

    #[test]
    fn u8_eta_expansion() {
        let () = check_eta_expansion::<u8>();
    }

    #[test]
    fn u8_serialization() {
        let () = check_serialization::<u8>();
    }

    #[test]
    fn deterministic_usize() {
        let () = register_globally::<usize>();
        let mut prng = WyRand::new(42);
        let expected: Vec<usize> = persist::replay()
            .chain([
                9,
                6,
                6,
                10_465_773_274_321_242_342,
                9_091_519_196_080_063_832,
                17_108_568_891_541_767_080,
                3,
                0,
                1,
                0,
            ])
            .collect();
        let generated: Vec<usize> = arbitrary(&mut prng).unwrap().take(expected.len()).collect();
        assert_eq!(generated, expected);
    }

    #[test]
    fn usize_eta_expansion() {
        let () = check_eta_expansion::<usize>();
    }

    #[test]
    fn usize_serialization() {
        let () = check_serialization::<usize>();
    }

    #[test]
    fn deterministic_uniform() {
        let mut prng = WyRand::new(42);
        assert_eq!(uniform(&mut prng), 0x_CA71_D87C_7698_3989);
        assert_eq!(uniform(&mut prng), 0x_7E5B_A615_5208_5FC6);
        assert_eq!(uniform(&mut prng), 0x_CDF1_01E3_BAB8_8B9F);
        assert_eq!(uniform(&mut prng), 0x_0A38_25AD_7326_7808);
        assert_eq!(uniform(&mut prng), 0x_8AC0_ADC1_5D67_1C29);
    }

    #[test]
    #[cfg(feature = "num-bigint")]
    fn big_uint_eta_expansion() {
        let () = check_eta_expansion::<num_bigint::BigUint>();
    }

    #[test]
    #[cfg(feature = "num-bigint")]
    fn big_uint_serialization() {
        let () = check_serialization::<num_bigint::BigUint>();
    }

    #[test]
    #[cfg(feature = "num-bigint")]
    fn deterministic_big_uint() {
        let mut prng = WyRand::new(42);
        let generated: Vec<num_bigint::BigUint> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<_> = [1, 9, 0, 0, 0, 3, 1, 0, 5, 0]
            .into_iter()
            .map(<num_bigint::BigUint as From<usize>>::from)
            .collect();
        assert_eq!(generated, expected);
    }

    #[test]
    #[cfg(feature = "num-bigint")]
    fn deterministic_big_uint_shrink() {
        use crate::{reflection::register_globally, shrink};
        let () = register_globally::<num_bigint::BigUint>();
        let orig: num_bigint::BigUint = 1_000_usize.into();
        let expected = [0, 500, 750, 875, 938, 969, 985, 993, 997, 999]
            .into_iter()
            .map(<num_bigint::BigUint as From<usize>>::from);
        let mut actual = shrink::candidates(orig);
        for expected_item in expected {
            assert_eq!(actual.next(), Some(expected_item));
        }
        assert_eq!(actual.next(), None);
    }
}
