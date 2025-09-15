//! Implementations for integer types (`i#`, `u#`, `NonZero<..>`).

use {
    crate::{
        ast_size::AstSize,
        error,
        exhaust::Exhaust,
        max::{Max, MaybeDecidable, MaybeOverflow},
        pseudorandom::Pseudorandom,
        test_impls_for,
        value_size::ValueSize,
    },
    core::{array, iter, num::NonZero},
    paste::paste,
    rand_core::RngCore,
};

/// Implement methods for `MaybeOverflow<$ty>`.
macro_rules! impl_maybe_overflow {
    ($ty:ty) => {
        impl MaybeOverflow<$ty> {
            #[inline]
            #[must_use]
            pub const fn minus(self, rhs: $ty) -> Self {
                match self {
                    Self::Contained(lhs) => match lhs.checked_sub(rhs) {
                        None => MaybeOverflow::Overflow,
                        Some(contained) => MaybeOverflow::Contained(contained),
                    },
                    Self::Overflow => Self::Overflow,
                }
            }

            #[inline]
            #[must_use]
            pub const fn minus_self(self, rhs: Self) -> Self {
                if let (Self::Contained(lhs), Self::Contained(rhs)) = (self, rhs) {
                    match lhs.checked_sub(rhs) {
                        None => MaybeOverflow::Overflow,
                        Some(contained) => MaybeOverflow::Contained(contained),
                    }
                } else {
                    Self::Overflow
                }
            }

            #[inline]
            pub const fn or_max(&self) -> $ty {
                match *self {
                    Self::Contained(contained) => contained,
                    Self::Overflow => <$ty>::MAX,
                }
            }

            #[inline]
            #[must_use]
            pub const fn plus(self, rhs: $ty) -> Self {
                match self {
                    Self::Contained(lhs) => match lhs.checked_add(rhs) {
                        None => MaybeOverflow::Overflow,
                        Some(contained) => MaybeOverflow::Contained(contained),
                    },
                    Self::Overflow => Self::Overflow,
                }
            }

            #[inline]
            #[must_use]
            pub const fn plus_self(self, rhs: Self) -> Self {
                if let (Self::Contained(lhs), Self::Contained(rhs)) = (self, rhs) {
                    match lhs.checked_add(rhs) {
                        None => MaybeOverflow::Overflow,
                        Some(contained) => MaybeOverflow::Contained(contained),
                    }
                } else {
                    Self::Overflow
                }
            }

            #[inline]
            pub const fn subtract_from(&self, lhs: $ty) -> $ty {
                match *self {
                    Self::Overflow => 0,
                    Self::Contained(rhs) => lhs.saturating_sub(rhs),
                }
            }
        }
    };
}

/// Implement methods for `Max<MaybeOverflow<$ty>>`.
macro_rules! impl_max {
    ($ty:ty) => {
        impl Max<MaybeOverflow<$ty>> {
            #[inline]
            #[must_use]
            pub const fn minus(self, rhs: $ty) -> Self {
                match self {
                    Self::Uninstantiable => Self::Uninstantiable,
                    Self::Finite(lhs) => Self::Finite(lhs.minus(rhs)),
                    Self::Infinite => Self::Infinite,
                }
            }

            #[inline]
            #[must_use]
            pub const fn plus(self, rhs: $ty) -> Self {
                match self {
                    Self::Uninstantiable => Self::Uninstantiable,
                    Self::Finite(lhs) => Self::Finite(lhs.plus(rhs)),
                    Self::Infinite => Self::Infinite,
                }
            }
        }
    };
}

/// Implement `AstSize for $ty`.
macro_rules! impl_ast_size {
    ($ty:ty) => {
        impl AstSize for $ty {
            const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));
            const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
                MaybeDecidable::Decidable(Max::Finite(0.));

            #[inline]
            fn ast_size(&self) -> MaybeOverflow<usize> {
                MaybeOverflow::Contained(0)
            }
        }
    };
}

/// Implement all relevant traits for `NonZero<$ty>`.
macro_rules! impl_nonzero {
    ($ty:ty) => {
        impl_ast_size!(NonZero<$ty>);

        impl ValueSize for NonZero<$ty> {
            const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                match <$ty as ValueSize>::MAX_VALUE_SIZE {
                    MaybeDecidable::Decidable(max) => MaybeDecidable::Decidable(max.minus(1)),
                    MaybeDecidable::AtMost(max) => MaybeDecidable::AtMost(max.minus(1)),
                };

            #[inline]
            fn value_size(&self) -> MaybeOverflow<usize> {
                <$ty as ValueSize>::value_size(&self.get()).minus(1)
            }
        }

        impl Exhaust for NonZero<$ty> {
            type Exhaust = iter::FilterMap<<$ty as Exhaust>::Exhaust, fn($ty) -> Option<Self>>;
            #[inline]
            fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
                let Some(value_size) = value_size.checked_add(1) else {
                    return Err(error::UnreachableSize);
                };
                <$ty as Exhaust>::exhaust(value_size).map(|iter| {
                    iter.filter_map({
                        #[expect(
                            clippy::as_conversions,
                            reason = "More stringently checked for function-pointer types"
                        )]
                        (Self::new as fn(_) -> _)
                    })
                })
            }
        }

        impl Pseudorandom for NonZero<$ty> {
            #[inline]
            fn pseudorandom<Rng: rand_core::RngCore>(
                expected_ast_size: f32,
                rng: &mut Rng,
            ) -> Result<Self, crate::error::Uninstantiable> {
                loop {
                    let maybe_zero = <$ty as Pseudorandom>::pseudorandom(expected_ast_size, rng)?;
                    if let Some(nonzero) = Self::new(maybe_zero) {
                        return Ok(nonzero);
                    }
                }
            }
        }

        paste! { test_impls_for!(NonZero<$ty>, [< nonzero_ $ty >]); }
    };
}

/// Implement logic common to all integers, signed or unsigned.
macro_rules! impl_int_for_ty {
    ($ty:ty) => {
        impl_maybe_overflow!($ty);
        impl_max!($ty);
        impl_ast_size!($ty);
        impl_nonzero!($ty);
        paste! { test_impls_for!($ty, [< $ty >]); }
    };
}

/// Implement logic common to all unsigned integers.
macro_rules! impl_unsigned {
    ($ty:ty) => {
        impl_int_for_ty!($ty);

        impl ValueSize for $ty {
            const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                MaybeDecidable::Decidable(Max::Finite({
                    #[expect(
                        clippy::allow_attributes,
                        clippy::as_conversions,
                        irrefutable_let_patterns,
                        reason = "Relevant only after a platform-dependent bit width."
                    )]
                    #[allow(clippy::cast_possible_truncation, reason = "Roundtrip checked.")]
                    if let cast = <$ty>::MAX as usize
                        && cast as $ty == <$ty>::MAX
                    {
                        MaybeOverflow::Contained(cast)
                    } else {
                        MaybeOverflow::Overflow
                    }
                }));

            #[inline]
            fn value_size(&self) -> MaybeOverflow<usize> {
                usize::try_from(*self).into()
            }
        }

        impl Exhaust for $ty {
            type Exhaust = iter::Once<Self>;
            #[inline]
            fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
                match Self::try_from(value_size) {
                    Ok(ok) => Ok(iter::once(ok)),
                    Err(_) => Err(error::UnreachableSize),
                }
            }
        }

        impl Pseudorandom for $ty {
            #[inline]
            fn pseudorandom<Rng: RngCore>(
                _expected_ast_size: f32,
                rng: &mut Rng,
            ) -> Result<Self, error::Uninstantiable> {
                #[expect(
                    clippy::allow_attributes,
                    clippy::as_conversions,
                    reason = "Relevant only after a platform-dependent bit width."
                )]
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_lossless,
                    reason = "Bit width checked above."
                )]
                let u32 = rng.next_u32() as Self;

                Ok(if const { <$ty>::BITS <= u32::BITS } {
                    u32
                } else {
                    let mut acc: Self = 0;
                    let mut shift: u32 = 0;
                    while let Some(shl) = u32.checked_shl(shift) {
                        acc |= shl;
                        shift = shift
                            .checked_add(32)
                            .expect("INTERNAL ERROR: Extremely wide integer!");
                    }
                    acc
                })
            }
        }

        impl MaybeOverflow<usize> {
            paste! {
                #[inline]
                pub const fn [< saturating_from_ $ty >](value: $ty) -> Self {
                    #[expect(
                        clippy::allow_attributes,
                        clippy::as_conversions,
                        reason = "Relevant only after a platform-dependent bit width."
                    )]
                    #[allow(clippy::cast_possible_truncation, reason = "still `0b11111...`")]
                    if value <= const { usize::MAX as $ty } {
                        Self::Contained(value as usize)
                    } else {
                        Self::Overflow
                    }
                }
            }
        }

        impl From<$ty> for MaybeOverflow<usize> {
            #[inline]
            fn from(value: $ty) -> Self {
                paste! { Self::[< saturating_from_ $ty >](value) }
            }
        }
    };
}

/// Implement logic common to all signed integers.
macro_rules! impl_signed {
    ($ty:ty) => {
        impl_int_for_ty!($ty);

        impl ValueSize for $ty {
            const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                MaybeDecidable::Decidable(Max::Finite({
                    if let Some(value_size_of_most_negative) =
                        <$ty>::MIN.unsigned_abs().checked_add(1)
                    {
                        #[expect(
                            clippy::allow_attributes,
                            clippy::as_conversions,
                            reason = "Relevant only after a platform-dependent bit width."
                        )]
                        #[allow(clippy::cast_possible_truncation, reason = "Roundtrip checked.")]
                        let cast = value_size_of_most_negative as usize;
                        if cast > 1 {
                            MaybeOverflow::Contained(cast)
                        } else {
                            MaybeOverflow::Overflow
                        }
                    } else {
                        MaybeOverflow::Overflow
                    }
                }));

            #[inline]
            fn value_size(&self) -> MaybeOverflow<usize> {
                MaybeOverflow::from(usize::try_from(self.unsigned_abs())).plus((*self < 0).into())
            }
        }

        impl Exhaust for $ty {
            type Exhaust = iter::Take<array::IntoIter<Self, 2>>;
            #[inline]
            fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
                const MAX_VALUE_SIZE: &MaybeOverflow<usize> =
                    <$ty>::MAX_VALUE_SIZE.at_most().unwrap_finite_ref();
                const ONE: $ty = 1;
                if let Ok(pos) = Self::try_from(value_size) {
                    // SAFETY: There's one more negative value than there are positive values,
                    // and the negative value we're computing is one fewer in absolute value.
                    let neg = unsafe { ONE.unchecked_sub(pos) };
                    Ok(if neg < 0 {
                        [neg, pos].into_iter().take(2)
                    } else {
                        [pos; 2].into_iter().take(1)
                    })
                } else {
                    if let MaybeOverflow::Contained(max_value_size) = *MAX_VALUE_SIZE
                        && value_size > max_value_size
                    {
                        return Err(error::UnreachableSize);
                    }
                    // SAFETY:
                    // Checked above, assuming `MAX_VALUE_SIZE` is correct (which is tested).
                    let neg_value_size_minus_one = unsafe { (!value_size).unchecked_add(2) };
                    #[expect(clippy::allow_attributes, reason = "Depends on `$ty`.")]
                    #[allow(
                        clippy::as_conversions,
                        clippy::cast_possible_wrap,
                        clippy::cast_possible_truncation,
                        reason = "Intentional."
                    )]
                    Ok([neg_value_size_minus_one as $ty; 2].into_iter().take(1))
                }
            }
        }

        impl Pseudorandom for $ty {
            #[inline]
            fn pseudorandom<Rng: RngCore>(
                _expected_ast_size: f32,
                rng: &mut Rng,
            ) -> Result<Self, error::Uninstantiable> {
                #[expect(
                    clippy::allow_attributes,
                    clippy::as_conversions,
                    reason = "Relevant only after a platform-dependent bit width."
                )]
                #[allow(
                    clippy::cast_possible_truncation,
                    clippy::cast_possible_wrap,
                    clippy::cast_lossless,
                    reason = "Bit width checked above."
                )]
                let u32 = rng.next_u32() as Self;

                Ok(if const { <$ty>::BITS <= u32::BITS } {
                    u32
                } else {
                    let mut acc: Self = 0;
                    let mut shift: u32 = 0;
                    while let Some(shl) = u32.checked_shl(shift) {
                        acc |= shl;
                        shift = shift
                            .checked_add(32)
                            .expect("INTERNAL ERROR: Extremely wide integer!");
                    }
                    acc
                })
            }
        }
    };
}

/// Implement logic common to all integers of a given bit width.
macro_rules! impl_int {
    ($bits:tt) => {
        paste! {
            impl_unsigned!([< u $bits >]);
            impl_signed!([< i $bits >]);
        }
    };
}

impl_int!(8);
impl_int!(16);
impl_int!(32);
impl_int!(64);
impl_int!(128);
impl_int!(size);

#[cfg(test)]
#[expect(
    clippy::panic,
    clippy::indexing_slicing,
    clippy::unwrap_used,
    reason = "Tests are supposed to fail if they don't behave as expected."
)]
mod test {
    extern crate alloc;

    use {
        crate::exhaust::{Exhaust as _, exhaust},
        alloc::{vec, vec::Vec},
    };

    #[test]
    fn exhaust_i8_128() {
        let exhaust: Vec<_> = i8::exhaust(128).unwrap().collect();
        assert_eq!(exhaust, vec![-127]);
    }

    #[test]
    fn exhaust_i8_129() {
        let exhaust: Vec<_> = i8::exhaust(129).unwrap().collect();
        assert_eq!(exhaust, vec![-128]);
    }

    #[test]
    fn exhaust_i8_130() {
        if let Ok(exhaust) = i8::exhaust(130) {
            let exhaust: Vec<_> = exhaust.collect();
            panic!("{exhaust:#?}");
        }
    }

    #[test]
    fn exhaust_i8() {
        let exhaust: Vec<i8> = exhaust().collect();
        assert_eq!(exhaust[..10], vec![0, 1, -1, 2, -2, 3, -3, 4, -4, 5]);
        assert_eq!(exhaust[250..], vec![-125, 126, -126, 127, -127, -128]);
    }

    /*
    #[test]
    fn exhaust_i16() {
        let exhaust: Vec<i16> = exhaust().collect();
        assert_eq!(exhaust[..10], vec![0, 1, -1, 2, -2, 3, -3, 4, -4, 5]);
        assert_eq!(
            exhaust[65_530..],
            vec![-32_765, 32_766, -32_766, 32_767, -32_767, -32_768],
        );
    }
    */
}
