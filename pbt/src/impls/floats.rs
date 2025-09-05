//! Implementations for floating-point types (`f32` and `f64`).

use crate::{
    ast_size::AstSize,
    impls::ints::in_between::{i11, u11, u23, u52},
    max::{Max, MaybeDecidable, MaybeOverflow},
    test_impls_for,
    value_size::ValueSize,
};

type F32Parts = (bool, i8, u23);
type F64Parts = (bool, i11, u52);

const F32_EXPONENT_BIAS: u8 = 127;
const F64_EXPONENT_BIAS: u11 = u11::new(1_023).unwrap();

#[inline]
const fn f32_from_parts((sign_bit, unbiased_exponent, backward_mantissa): F32Parts) -> f32 {
    let biased_exponent = (unbiased_exponent as u8).wrapping_sub(F32_EXPONENT_BIAS) as u32;
    let forward_mantissa = backward_mantissa.reverse_bits();

    let bits = ((sign_bit as u32) << 31_u32) | (biased_exponent << 23_u32) | forward_mantissa.get();
    f32::from_bits(bits)
}

#[inline]
const fn f64_from_parts((sign_bit, unbiased_exponent, backward_mantissa): F64Parts) -> f64 {
    let biased_exponent = unbiased_exponent
        .as_unsigned()
        .wrapping_sub(F64_EXPONENT_BIAS)
        .get() as u64;
    let forward_mantissa = backward_mantissa.reverse_bits();

    let bits = ((sign_bit as u64) << 63_u32) | (biased_exponent << 52_u32) | forward_mantissa.get();
    f64::from_bits(bits)
}

#[inline]
const fn parts_from_f32(f: f32) -> F32Parts {
    let bits = f.to_bits();

    let sign_bit = (bits & const { 1 << 31 }) != 0;
    let biased_exponent = (bits >> 23_u32) as u8;
    let forward_mantissa = u23::new_masking(bits);

    let unbiased_exponent = biased_exponent.wrapping_add(F32_EXPONENT_BIAS) as i8;
    let backward_mantissa = forward_mantissa.reverse_bits();

    (sign_bit, unbiased_exponent, backward_mantissa)
}

#[inline]
const fn parts_from_f64(f: f64) -> F64Parts {
    let bits = f.to_bits();

    let sign_bit = (bits & const { 1 << 63 }) != 0;
    let biased_exponent = u11::new_masking((bits >> 52_u32) as u16);
    let forward_mantissa = u52::new_masking(bits);

    let unbiased_exponent = biased_exponent.wrapping_add(F64_EXPONENT_BIAS).as_signed();
    let backward_mantissa = forward_mantissa.reverse_bits();

    (sign_bit, unbiased_exponent, backward_mantissa)
}

impl AstSize for f32 {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<Self>> =
        MaybeDecidable::Decidable(Max::Finite(0.));

    #[inline]
    fn ast_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(0)
    }
}

impl AstSize for f64 {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
        MaybeDecidable::Decidable(Max::Finite(0.));

    #[inline]
    fn ast_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(0)
    }
}

impl ValueSize for f32 {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));

    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(0)
    }
}

impl ValueSize for f64 {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));

    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(0)
    }
}

test_impls_for!(f32, f32);
test_impls_for!(f64, f64);

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{
            exhaust::exhaust,
            pseudorandom::{default_rng, pseudorandom},
        },
        alloc::{vec, vec::Vec},
    };

    extern crate alloc;

    #[test]
    fn exhaust_f32() {
        let exhaust: Vec<f32> = exhaust().take(10).collect();
        assert_eq!(exhaust, vec![1.0; 10]);
    }

    #[test]
    fn exhaust_f64() {
        let exhaust: Vec<f64> = exhaust().take(10).collect();
        assert_eq!(exhaust, vec![1.0; 10]);
    }

    #[test]
    fn f32_parts_roundtrip() {
        let mut rng = default_rng();
        for parts in pseudorandom(&mut rng) {
            let f = f32_from_parts(parts);
            let roundtrip = parts_from_f32(f);
            assert_eq!(
                parts, roundtrip,
                "{parts:?} -> {f:?} -> {roundtrip:?} =/= {parts:?}",
            );
        }
    }

    #[test]
    fn f64_parts_roundtrip() {
        let mut rng = default_rng();
        for parts in pseudorandom(&mut rng) {
            let f = f64_from_parts(parts);
            let roundtrip = parts_from_f64(f);
            assert_eq!(
                parts, roundtrip,
                "{parts:?} -> {f:?} -> {roundtrip:?} =/= {parts:?}",
            );
        }
    }
}
