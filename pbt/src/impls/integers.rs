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
            let mut bit_reservoir = prng.rand();
            let mut remaining_bits: u8 = 64;
            let mut coin_flip = || -> bool {
                if let Some(decrement) = remaining_bits.checked_sub(1) {
                    remaining_bits = decrement;
                } else {
                    bit_reservoir = prng.rand();
                    remaining_bits = 63;
                }
                let bit = (bit_reservoir & 1) != 0;
                bit_reservoir >>= 1_u8;
                bit
            };

            if coin_flip() {
                return 0;
            }
            let mut acc: $u = 1;
            while coin_flip() {
                acc = acc.wrapping_shl(1) | <$u>::from(coin_flip());
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
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn u8_deterministic() {
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
    fn usize_deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<usize> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected = vec![
            42, // <-- persisted to `.pbt/` and replayed
            9,
            6,
            6,
            10_465_773_274_321_242_342,
            9_091_519_196_080_063_832,
            17_108_568_891_541_767_080,
            3,
            0,
            1,
        ];
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
}
