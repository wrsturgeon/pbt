//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use crate::{
    Pbt,
    fields::{Fields, Store},
    reflection::{Parts, Reflection, Variant},
    registration::Registration,
};

impl Pbt for usize {
    #[inline]
    fn construct<F>(Parts { mut fields, .. }: Parts<F>) -> Self
    where
        F: Fields,
    {
        fields.field()
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: 0,
        }
    }

    #[inline]
    fn register(_registration: &mut Registration<'_>) -> Reflection<Self> {
        Reflection {
            variants: vec![
                // This generator samples uniformly on [0, usize::MAX]:
                Variant::Literal {
                    generator: |prng| {
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
                    },
                },
                // This generator samples "small" values with coin flips for each bit:
                Variant::Literal {
                    generator: |prng| {
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
                        let mut acc: usize = 1;
                        #[expect(
                            clippy::as_conversions,
                            clippy::cast_lossless,
                            reason = "truncation is impossible: `usize` can't be 1 bit and run Rust"
                        )]
                        while coin_flip() {
                            acc = acc.wrapping_shl(1) | (coin_flip() as usize);
                        }
                        acc
                    },
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        crate::{arbitrary, check_eta_expansion},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<usize> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected = vec![
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
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<usize>();
    }
}
