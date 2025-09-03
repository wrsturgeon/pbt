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
    core::{iter, num::NonZero},
    paste::paste,
    rand_core::RngCore,
};

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
        }
    };
}

macro_rules! impl_max {
    ($ty:ty) => {
        impl Max<MaybeOverflow<$ty>> {
            #[inline]
            pub const fn plus(self, rhs: $ty) -> Self {
                match self {
                    Self::Uninstantiable => Self::Uninstantiable,
                    Self::Finite(lhs) => Self::Finite(lhs.plus(rhs)),
                    Self::Infinite => Self::Infinite,
                }
            }

            #[inline]
            pub const fn minus(self, rhs: $ty) -> Self {
                match self {
                    Self::Uninstantiable => Self::Uninstantiable,
                    Self::Finite(lhs) => Self::Finite(lhs.minus(rhs)),
                    Self::Infinite => Self::Infinite,
                }
            }
        }
    };
}

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

macro_rules! impl_nonzero {
    ($ty:ty) => {
        impl_ast_size!(NonZero<$ty>);

        impl ValueSize for NonZero<$ty> {
            const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                match <$ty as ValueSize>::MAX_VALUE_SIZE {
                    MaybeDecidable::Undecidable => MaybeDecidable::Undecidable,
                    MaybeDecidable::Decidable(max) => MaybeDecidable::Decidable(max.minus(1)),
                };

            #[inline]
            fn value_size(&self) -> MaybeOverflow<usize> {
                <$ty as ValueSize>::value_size(&self.get()).minus(1)
            }
        }

        impl Exhaust for NonZero<$ty> {
            #[inline]
            fn exhaust(
                value_size: usize,
            ) -> Result<impl Iterator<Item = Self>, error::UnreachableSize> {
                let Some(value_size) = value_size.checked_add(1) else {
                    return Err(error::UnreachableSize);
                };
                <$ty as Exhaust>::exhaust(value_size).map(|iter| iter.filter_map(Self::new))
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

macro_rules! impl_int_for_ty {
    ($ty:ty) => {
        impl_maybe_overflow!($ty);
        impl_max!($ty);
        impl_ast_size!($ty);
        impl_nonzero!($ty);
        paste! { test_impls_for!($ty, [< $ty >]); }
    };
}

macro_rules! impl_unsigned {
    ($ty:ty) => {
        impl_int_for_ty!($ty);

        impl ValueSize for $ty {
            const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                MaybeDecidable::Decidable(Max::Finite({
                    let cast = <$ty>::MAX as usize;
                    if cast as $ty == <$ty>::MAX {
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
            #[inline]
            fn exhaust(
                value_size: usize,
            ) -> Result<impl Iterator<Item = Self>, error::UnreachableSize> {
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
                Ok(if const { <$ty>::BITS <= u32::BITS } {
                    rng.next_u32() as Self
                } else {
                    let mut acc: Self = 0;
                    let mut shift: u32 = 0;
                    while let Some(shl) = (rng.next_u32() as Self).checked_shl(shift) {
                        acc |= shl;
                        shift += 32;
                    }
                    acc
                })
            }
        }
    };
}

macro_rules! impl_signed {
    ($ty:ty) => {
        impl_int_for_ty!($ty);

        impl ValueSize for $ty {
            const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                MaybeDecidable::Decidable(Max::Finite({
                    let cast = <$ty>::MIN.unsigned_abs() as usize;
                    if cast == 0 {
                        MaybeOverflow::Overflow
                    } else {
                        MaybeOverflow::Contained(cast)
                    }
                }));

            #[inline]
            fn value_size(&self) -> MaybeOverflow<usize> {
                MaybeOverflow::from(usize::try_from(self.unsigned_abs())).plus((*self < 0).into())
            }
        }

        impl Exhaust for $ty {
            #[inline]
            fn exhaust(
                value_size: usize,
            ) -> Result<impl Iterator<Item = Self>, error::UnreachableSize> {
                const ONE: $ty = 1;
                match Self::try_from(value_size) {
                    Ok(pos) => {
                        // SAFETY: There's one more negative value than there are positive values,
                        // and the negative value we're computing is one fewer in absolute value.
                        let neg = unsafe { ONE.unchecked_sub(pos) };
                        Ok([pos, neg].into_iter().take(if neg < 0 { 2 } else { 1 }))
                    }
                    Err(_) => {
                        // SAFETY: If `value_size` were zero, the above would have succeeded.
                        let value_size = unsafe { value_size.unchecked_sub(1) };
                        if value_size == const { Self::MIN.unsigned_abs() as usize } {
                            Ok([Self::MIN, Self::MIN].into_iter().take(1))
                        } else {
                            Err(error::UnreachableSize)
                        }
                    }
                }
            }
        }

        impl Pseudorandom for $ty {
            #[inline]
            fn pseudorandom<Rng: RngCore>(
                _expected_ast_size: f32,
                rng: &mut Rng,
            ) -> Result<Self, error::Uninstantiable> {
                Ok(if const { <$ty>::BITS <= u32::BITS } {
                    rng.next_u32() as Self
                } else {
                    let mut acc: Self = 0;
                    let mut shift: u32 = 0;
                    while let Some(shl) = (rng.next_u32() as Self).checked_shl(shift) {
                        acc |= shl;
                        shift += 32;
                    }
                    acc
                })
            }
        }
    };
}

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
