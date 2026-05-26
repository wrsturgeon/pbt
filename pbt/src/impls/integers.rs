//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        pbt::Pbt,
        reflection::{Erased, Variant},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::any::TypeId,
};

impl Pbt for usize {
    #[inline]
    fn variants(
        _variants: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>,
        visited: &mut HashSet<TypeId>,
    ) -> Arc<[Variant<Self>]> {
        let ty = TypeId::of::<Self>();
        if visited.insert(ty) {
            // here's where we'd run DFS iff not already in `visited`
        }
        Arc::new([
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
        ])
    }
}
