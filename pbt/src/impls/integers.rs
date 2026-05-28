//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        Pbt,
        fields::Fields,
        reflection::{Constructor, Erased, Variant},
    },
    ahash::HashSet,
    alloc::{collections::BTreeMap, sync::Arc},
    core::any::TypeId,
};

impl Pbt for usize {
    #[inline]
    #[expect(clippy::panic, reason = "end-users shouldn't be calling this")]
    fn instantiate_variant<F>(_variant_index: usize, _fields: F) -> Self
    where
        F: Fields,
    {
        panic!("can't call `usize::instantiate_variant`: `usize` is a literal type")
    }

    #[inline]
    fn variants(
        _variants: &mut BTreeMap<TypeId, Arc<[Constructor<Erased>]>>,
        _visited: &mut HashSet<TypeId>,
    ) -> Vec<Variant<Self>> {
        vec![
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
        ]
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {crate::arbitrary, pretty_assertions::assert_eq, wyrand::WyRand};

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
}
