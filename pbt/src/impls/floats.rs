//! Implementations for floating-point types (`f32` and `f64`).

use core::iter;

use crate::{
    ast_size::AstSize,
    error,
    exhaust::Exhaust,
    impls::ints::in_between::{i11, u11, u23, u52},
    max::{Max, MaybeDecidable, MaybeOverflow},
    pseudorandom::Pseudorandom,
    test_impls_for,
    value_size::ValueSize,
};

/// Exponent bits (in memory) at which the effective exponent (of the number represented) is zero.
const F32_EXPONENT_BIAS: u8 = 127;
/// Exponent bits (in memory) at which the effective exponent (of the number represented) is zero.
const F64_EXPONENT_BIAS: u11 = u11::new(1_023).unwrap();

/// In-memory representation of a floating-point number, split into a tuple.
type F32Parts = (i8, u23, bool);
/// In-memory representation of a floating-point number, split into a tuple.
type F64Parts = (i11, u52, bool);

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
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = F32Parts::MAX_VALUE_SIZE;

    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        parts_from_f32(*self).value_size()
    }
}

impl ValueSize for f64 {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = F64Parts::MAX_VALUE_SIZE;

    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        parts_from_f64(*self).value_size()
    }
}

impl Exhaust for f32 {
    type Exhaust = iter::Map<<F32Parts as Exhaust>::Exhaust, fn(F32Parts) -> Self>;
    #[inline]
    fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
        Ok(<F32Parts as Exhaust>::exhaust(value_size)?.map(f32_from_parts))
    }
}

impl Exhaust for f64 {
    type Exhaust = iter::Map<<F64Parts as Exhaust>::Exhaust, fn(F64Parts) -> Self>;
    #[inline]
    fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
        Ok(<F64Parts as Exhaust>::exhaust(value_size)?.map(f64_from_parts))
    }
}

impl Pseudorandom for f32 {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        _expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        Ok(Self::from_bits(rng.next_u32()))
    }
}

impl Pseudorandom for f64 {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        _expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        Ok(Self::from_bits(rng.next_u64()))
    }
}

test_impls_for!(F32Parts, f32_parts);
// test_impls_for!(F64Parts, f64_parts);
test_impls_for!(f32, f32);
// test_impls_for!(f64, f64);

/// Build a floating-point number from each part of its in-memory representation.
#[inline]
const fn f32_from_parts((unbiased_exponent, backward_mantissa, sign_bit): F32Parts) -> f32 {
    #[expect(
        clippy::as_conversions,
        clippy::cast_sign_loss,
        reason = "Wrapping and sign loss are intentional, and each type is at least as large as the last."
    )]
    let biased_exponent = (unbiased_exponent as u8).wrapping_add(F32_EXPONENT_BIAS) as u32;
    let forward_mantissa = backward_mantissa.reverse_bits();

    #[expect(
        clippy::as_conversions,
        reason = "`From` is not yet `const`: this is just interpreting a Boolean as an integer."
    )]
    let bits = ((sign_bit as u32) << 31_u32) | (biased_exponent << 23_u32) | forward_mantissa.get();
    f32::from_bits(bits)
}

/// Build a floating-point number from each part of its in-memory representation.
#[inline]
const fn f64_from_parts((unbiased_exponent, backward_mantissa, sign_bit): F64Parts) -> f64 {
    #[expect(
        clippy::as_conversions,
        reason = "Wrapping and sign loss are intentional, and each type is at least as large as the last."
    )]
    let biased_exponent = unbiased_exponent
        .as_unsigned()
        .wrapping_add(F64_EXPONENT_BIAS)
        .get() as u64;
    let forward_mantissa = backward_mantissa.reverse_bits();

    #[expect(
        clippy::as_conversions,
        reason = "`From` is not yet `const`: this is just interpreting a Boolean as an integer."
    )]
    let bits = ((sign_bit as u64) << 63_u32) | (biased_exponent << 52_u32) | forward_mantissa.get();
    f64::from_bits(bits)
}

/// Split a floating-point number into each part of its in-memory representation.
#[inline]
const fn parts_from_f32(float: f32) -> F32Parts {
    let bits = float.to_bits();

    let sign_bit = (bits & const { 1_u32 << 31_u32 }) != 0;
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "Intentional: 32 - 23 = 9, and the ninth is the sign bit."
    )]
    let biased_exponent = (bits >> 23_u32) as u8;
    let forward_mantissa = u23::new_masking(bits);

    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_wrap,
        reason = "Intentional: interpret the MSB as a sign bit."
    )]
    let unbiased_exponent = biased_exponent.wrapping_sub(F32_EXPONENT_BIAS) as i8;
    let backward_mantissa = forward_mantissa.reverse_bits();

    (unbiased_exponent, backward_mantissa, sign_bit)
}

/// Split a floating-point number into each part of its in-memory representation.
#[inline]
const fn parts_from_f64(float: f64) -> F64Parts {
    let bits = float.to_bits();

    let sign_bit = (bits & const { 1_u64 << 63_u32 }) != 0;
    #[expect(
        clippy::as_conversions,
        reason = "Intentional: to slice off the sign bit."
    )]
    let biased_exponent = u11::new_masking((bits >> 52_u32) as u16);
    let forward_mantissa = u52::new_masking(bits);

    let unbiased_exponent = biased_exponent.wrapping_sub(F64_EXPONENT_BIAS).as_signed();
    let backward_mantissa = forward_mantissa.reverse_bits();

    (unbiased_exponent, backward_mantissa, sign_bit)
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "Tests are supposed to fail if they don't behave as expected."
)]
mod test {
    extern crate alloc;

    use {
        super::*,
        crate::{
            exhaust::exhaust,
            pseudorandom::{default_rng, pseudorandom},
        },
        alloc::{vec, vec::Vec},
    };

    #[test]
    fn exhaust_f32_parts() {
        let exhaust: Vec<F32Parts> = exhaust().take(100).collect();
        assert_eq!(
            exhaust[..9],
            vec![
                (0, u23::new(0).unwrap(), false),
                (1, u23::new(0).unwrap(), false),
                (0, u23::new(1).unwrap(), false),
                (0, u23::new(0).unwrap(), true),
                (-1, u23::new(0).unwrap(), false),
                (2, u23::new(0).unwrap(), false),
                (1, u23::new(1).unwrap(), false),
                (1, u23::new(0).unwrap(), true),
                (0, u23::new(2).unwrap(), false)
            ]
        );
    }

    #[test]
    fn exhaust_f32() {
        let exhaust: Vec<f32> = exhaust().take(100).collect();
        assert_eq!(
            exhaust,
            vec![
                1.0, 2.0, 1.5, -1.0, 0.5, 4.0, 3.0, -2.0, 1.25, -1.5, 0.25, 8.0, 0.75, -0.5, 6.0,
                -4.0, 2.5, -3.0, 1.75, -1.25, 0.125, 16.0, 0.375, -0.25, 12.0, -8.0, 0.625, -0.75,
                5.0, -6.0, 3.5, -2.5, 1.125, -1.75, 0.0625, 32.0, 0.1875, -0.125, 24.0, -16.0,
                0.3125, -0.375, 10.0, -12.0, 0.875, -0.625, 7.0, -5.0, 2.25, -3.5, 1.625, -1.125,
                0.03125, 64.0, 0.09375, -0.0625, 48.0, -32.0, 0.15625, -0.1875, 20.0, -24.0,
                0.4375, -0.3125, 14.0, -10.0, 0.5625, -0.875, 4.5, -7.0, 3.25, -2.25, 1.375,
                -1.625, 0.015625, 128.0, 0.046875, -0.03125, 96.0, -64.0, 0.078125, -0.09375, 40.0,
                -48.0, 0.21875, -0.15625, 28.0, -20.0, 0.28125, -0.4375, 9.0, -14.0, 0.8125,
                -0.5625, 6.5, -4.5, 2.75, -3.25, 1.875, -1.375
            ]
        );
    }

    #[test]
    fn exhaust_f64() {
        let exhaust: Vec<f64> = exhaust().take(100).collect();
        assert_eq!(
            exhaust,
            vec![
                1.0, 2.0, 1.5, -1.0, 0.5, 4.0, 3.0, -2.0, 1.25, -1.5, 0.25, 8.0, 0.75, -0.5, 6.0,
                -4.0, 2.5, -3.0, 1.75, -1.25, 0.125, 16.0, 0.375, -0.25, 12.0, -8.0, 0.625, -0.75,
                5.0, -6.0, 3.5, -2.5, 1.125, -1.75, 0.0625, 32.0, 0.1875, -0.125, 24.0, -16.0,
                0.3125, -0.375, 10.0, -12.0, 0.875, -0.625, 7.0, -5.0, 2.25, -3.5, 1.625, -1.125,
                0.03125, 64.0, 0.09375, -0.0625, 48.0, -32.0, 0.15625, -0.1875, 20.0, -24.0,
                0.4375, -0.3125, 14.0, -10.0, 0.5625, -0.875, 4.5, -7.0, 3.25, -2.25, 1.375,
                -1.625, 0.015625, 128.0, 0.046875, -0.03125, 96.0, -64.0, 0.078125, -0.09375, 40.0,
                -48.0, 0.21875, -0.15625, 28.0, -20.0, 0.28125, -0.4375, 9.0, -14.0, 0.8125,
                -0.5625, 6.5, -4.5, 2.75, -3.25, 1.875, -1.375
            ]
        );
    }

    #[test]
    fn f32_parts_roundtrip() {
        let mut rng = default_rng();
        for parts in pseudorandom(&mut rng).take(10_000) {
            let float = f32_from_parts(parts);
            let roundtrip = parts_from_f32(float);
            assert_eq!(
                parts, roundtrip,
                "{parts:?} -> {float:?} -> {roundtrip:?} =/= {parts:?}",
            );
        }
    }

    #[test]
    fn f64_parts_roundtrip() {
        let mut rng = default_rng();
        for parts in pseudorandom(&mut rng).take(10_000) {
            let float = f64_from_parts(parts);
            let roundtrip = parts_from_f64(float);
            assert_eq!(
                parts, roundtrip,
                "{parts:?} -> {float:?} -> {roundtrip:?} =/= {parts:?}",
            );
        }
    }
}
