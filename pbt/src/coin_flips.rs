//! Cached coin flips backed by `WyRand`.

use wyrand::WyRand;

/// A sequence of coin flips, powered by a pseudorandom number generator.
pub(crate) struct CoinFlips {
    /// Cached bits from a full `u64` PRNG generation.
    bit_reservoir: u64,
    /// The number of remaining cached bits from a full `u64` PRNG generation.
    remaining_bits: u8,
}

impl CoinFlips {
    /// Flip a coin: sample `bool` with equal probability of `true` or `false`.
    #[inline]
    pub(crate) fn flip(&mut self, prng: &mut WyRand) -> bool {
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
    pub(crate) fn new(prng: &mut WyRand) -> Self {
        Self {
            bit_reservoir: prng.rand(),
            remaining_bits: 64,
        }
    }

    /// Flip `2^n` coins and return whether *any* of them were `true`.
    #[inline]
    #[cfg(feature = "num-bigint")]
    pub(crate) fn pow2_flips(&mut self, prng: &mut WyRand, pow2: usize) -> bool {
        for _ in 0..pow2 {
            if self.flip(prng) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use {super::*, core::iter, pretty_assertions::assert_eq};

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let mut coin = CoinFlips::new(&mut prng);
        let generated: Vec<bool> = iter::repeat_with(|| coin.flip(&mut prng))
            .take(16_usize)
            .collect();
        let expected = vec![
            true, false, false, true, false, false, false, true, true, false, false, true, true,
            true, false, false,
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    #[cfg(feature = "num-bigint")]
    fn deterministic_pow2_flips() {
        let mut prng = WyRand::new(42);
        let mut coin = CoinFlips::new(&mut prng);
        let generated: Vec<bool> = (0_usize..8_usize)
            .map(|pow2| coin.pow2_flips(&mut prng, pow2))
            .collect();
        assert_eq!(
            generated,
            vec![false, true, false, true, true, true, true, true],
        );
    }
}
