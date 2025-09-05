//! Implementations for integer types (`i#`, `u#`, `NonZero<..>`)
//! between powers of two (e.g. `u7`, not `u8`).

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
    paste::paste,
    rand_core::RngCore,
};

/// Implement an integer logically consisting of the first number of bits
/// but backed by the second number of bits in hardware.
macro_rules! impl_int_in_between {
    ($partial:tt, $full:tt) => {
        paste! {
            #[expect(non_camel_case_types, reason = "To match built-in integers.")]
            #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct [< u $partial >]([< u $full >]);

            #[expect(non_camel_case_types, reason = "To match built-in integers.")]
            #[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
            pub struct [< i $partial >]([< i $full >]);

            impl [< u $partial >] {
                pub const MAX: Self = Self(Self::[< MAX_U $full >]);
                pub const [< MAX_U $full >]: [< u $full >] = (1 << $partial) - 1;

                #[inline]
                pub const fn get(self) -> [< u $full >] {
                    self.0
                }

                #[inline]
                pub const fn new(full: [< u $full >]) -> Option<Self> {
                    if full <= Self::[< MAX_U $full >] {
                        Some(Self(full))
                    } else {
                        None
                    }
                }
            }

            impl [< i $partial >] {
                pub const MAX: Self = Self(Self::[< MAX_I $full >]);
                pub const [< MAX_I $full >]: [< i $full >] =
                    if let Some(bits) = [< $partial _u32 >].checked_sub(1) {
                        (1 << bits) - 1
                    } else {
                        0
                    };
                pub const MIN: Self = Self(Self::[< MIN_I $full >]);
                pub const [< MIN_I $full >]: [< i $full >] =
                    if let Some(bits) = [< $partial _u32 >].checked_sub(1) {
                        -(1 << bits)
                    } else {
                        0
                    };

                #[inline]
                pub const fn get(self) -> [< i $full >] {
                    self.0
                }

                #[inline]
                pub const fn new(full: [< i $full >]) -> Option<Self> {
                    if Self::[< MIN_I $full >] <= full && full <= Self::[< MAX_I $full >] {
                        Some(Self(full))
                    } else {
                        None
                    }
                }

                #[inline]
                pub const fn new_unchecked(full: [< i $full >]) -> Self {
                    #[cfg(test)]
                    {
                        assert!(
                            Self::[< MIN_I $full >] <= full && full <= Self::[< MAX_I $full >],
                            // "`new_unchecked({full:?})` out of range: should satisfy {:?} <= {full:?} <= {:?}",
                            // Self::[< MIN_I $full >],
                            // Self::[< MAX_I $full >],
                        );
                    }
                    Self(full)
                }
            }

            impl AstSize for [< u $partial >] {
                const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                    MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));
                const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
                    MaybeDecidable::Decidable(Max::Finite(0.));

                #[inline]
                fn ast_size(&self) -> MaybeOverflow<usize> {
                    MaybeOverflow::Contained(0)
                }
            }

            impl AstSize for [< i $partial >] {
                const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                    MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));
                const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
                    MaybeDecidable::Decidable(Max::Finite(0.));

                #[inline]
                fn ast_size(&self) -> MaybeOverflow<usize> {
                    MaybeOverflow::Contained(0)
                }
            }

            impl ValueSize for [< u $partial >] {
                const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                    MaybeDecidable::Decidable(Max::Finite(
                        if let Some(shl) = 1_usize.checked_shl($partial) {
                            MaybeOverflow::Contained(shl - 1)
                        } else {
                            MaybeOverflow::Overflow
                        }
                    ));

                #[inline]
                fn value_size(&self) -> MaybeOverflow<usize> {
                    self.0.value_size()
                }
            }

            impl ValueSize for [< i $partial >] {
                const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
                    MaybeDecidable::Decidable(Max::Finite(
                        if let Some(bits) = [< $partial _u32 >].checked_sub(1) {
                            if let Some(shl) = 1_usize.checked_shl(bits) {
                                MaybeOverflow::Contained(shl).plus(1)
                            } else {
                                MaybeOverflow::Overflow
                            }
                        } else {
                            MaybeOverflow::Overflow
                        }
                    ));

                #[inline]
                fn value_size(&self) -> MaybeOverflow<usize> {
                    self.0.value_size()
                }
            }

            impl Exhaust for [< u $partial >] {
                #[inline]
                fn exhaust(value_size: usize) -> Result<impl Iterator<Item = Self>, error::UnreachableSize> {
                    #[expect(
                        clippy::allow_attributes,
                        clippy::as_conversions,
                        reason = "Relevant only after a platform-dependent bit width."
                    )]
                    #[allow(clippy::cast_possible_truncation, reason = "still `0b11111...`")]
                    if value_size <= const { Self::[< MAX_U $full >] as usize } {
                        // SAFETY:
                        // `[< Self::MAX_U $full >]` is always going to be
                        // less than `[< u $full >]::MAX`, so this will be reachable.
                        let iter = unsafe { [< u $full >]::exhaust(value_size).unwrap_unchecked() };
                        // SAFETY:
                        // Checked above.
                        Ok(iter.map(|internal| unsafe { Self::new(internal).unwrap_unchecked() }))
                    } else {
                        Err(error::UnreachableSize)
                    }
                }
            }

            impl Exhaust for [< i $partial >] {
                #[inline]
                fn exhaust(value_size: usize) -> Result<impl Iterator<Item = Self>, error::UnreachableSize> {
                    const MAX_VALUE_SIZE: &MaybeOverflow<usize> =
                        <[< i $partial >]>::MAX_VALUE_SIZE.unwrap_ref().unwrap_finite_ref();
                    if value_size <= const { Self::[< MAX_I $full >] as usize } {
                        let pos = Self::new_unchecked(value_size as [< i $full >]);
                        // SAFETY: There's one more negative value than there are positive values,
                        // and the negative value we're computing is one fewer in absolute value.
                        let neg = unsafe { [< 1_i $full >].unchecked_sub(pos.0) };
                        Ok(if neg < 0 {
                            [Self::new_unchecked(neg), pos].into_iter().take(2)
                        } else {
                            [pos; 2].into_iter().take(1)
                        })
                    } else if let MaybeOverflow::Contained(max_value_size) = *MAX_VALUE_SIZE
                        && value_size > max_value_size
                    {
                        Err(error::UnreachableSize)
                    } else {
                        let Some(neg_value_size_minus_one) = (!value_size).checked_add(2) else {
                            return Ok([Self::new_unchecked(0); 2].into_iter().take(0)); // empty
                        };
                        let Some(neg_value_size_minus_one) = Self::new(neg_value_size_minus_one as [< i $full >]) else {
                            return Err(error::UnreachableSize);
                        };
                        Ok([neg_value_size_minus_one; 2].into_iter().take(1))
                    }
                }
            }

            impl Pseudorandom for [< u $partial >] {
                #[inline]
                fn pseudorandom<Rng: RngCore>(
                    expected_ast_size: f32,
                    rng: &mut Rng,
                ) -> Result<Self, error::Uninstantiable> {
                    let full = unsafe { <[< u $full >] as Pseudorandom>::pseudorandom(expected_ast_size, rng).unwrap_unchecked() };
                    Ok(unsafe { Self::new(full & Self::[< MAX_U $full >]).unwrap_unchecked() })
                }
            }

            impl Pseudorandom for [< i $partial >] {
                #[inline]
                fn pseudorandom<Rng: RngCore>(
                    expected_ast_size: f32,
                    rng: &mut Rng,
                ) -> Result<Self, error::Uninstantiable> {
                    // SAFETY:
                    // Integers are instantiable.
                    let full = unsafe { <[< i $full >] as Pseudorandom>::pseudorandom(expected_ast_size, rng).unwrap_unchecked() };
                    let internal = if full < [< 0_i $full >] {
                        if const { [< $full _u32 >] == 0_u32 } {
                            full | Self::[< MIN_I $full >]
                        } else {
                            [< 0_i $full >]
                        }
                    } else {
                        full & Self::[< MAX_I $full >]
                    };
                    Ok(unsafe { Self::new(internal).unwrap_unchecked() })
                }
            }

            test_impls_for!([< u $partial >], [< u_ $partial >]);
            test_impls_for!([< i $partial >], [< i_ $partial >]);
        }
    };
}

impl_int_in_between!(0, 8);
impl_int_in_between!(1, 8);
impl_int_in_between!(2, 8);
impl_int_in_between!(3, 8);
impl_int_in_between!(4, 8);
impl_int_in_between!(5, 8);
impl_int_in_between!(6, 8);
impl_int_in_between!(7, 8);
impl_int_in_between!(9, 16);
impl_int_in_between!(10, 16);
impl_int_in_between!(11, 16);
impl_int_in_between!(12, 16);
impl_int_in_between!(13, 16);
impl_int_in_between!(14, 16);
impl_int_in_between!(15, 16);
impl_int_in_between!(17, 32);
impl_int_in_between!(18, 32);
impl_int_in_between!(19, 32);
impl_int_in_between!(20, 32);
impl_int_in_between!(21, 32);
impl_int_in_between!(22, 32);
impl_int_in_between!(23, 32);
impl_int_in_between!(24, 32);
impl_int_in_between!(25, 32);
impl_int_in_between!(26, 32);
impl_int_in_between!(27, 32);
impl_int_in_between!(28, 32);
impl_int_in_between!(29, 32);
impl_int_in_between!(30, 32);
impl_int_in_between!(31, 32);
impl_int_in_between!(33, 64);
impl_int_in_between!(34, 64);
impl_int_in_between!(35, 64);
impl_int_in_between!(36, 64);
impl_int_in_between!(37, 64);
impl_int_in_between!(38, 64);
impl_int_in_between!(39, 64);
impl_int_in_between!(40, 64);
impl_int_in_between!(41, 64);
impl_int_in_between!(42, 64);
impl_int_in_between!(43, 64);
impl_int_in_between!(44, 64);
impl_int_in_between!(45, 64);
impl_int_in_between!(46, 64);
impl_int_in_between!(47, 64);
impl_int_in_between!(48, 64);
impl_int_in_between!(49, 64);
impl_int_in_between!(50, 64);
impl_int_in_between!(51, 64);
impl_int_in_between!(52, 64);
impl_int_in_between!(53, 64);
impl_int_in_between!(54, 64);
impl_int_in_between!(55, 64);
impl_int_in_between!(56, 64);
impl_int_in_between!(57, 64);
impl_int_in_between!(58, 64);
impl_int_in_between!(59, 64);
impl_int_in_between!(60, 64);
impl_int_in_between!(61, 64);
impl_int_in_between!(62, 64);
impl_int_in_between!(63, 64);
/*
impl_int_in_between!(65, 128);
impl_int_in_between!(66, 128);
impl_int_in_between!(67, 128);
impl_int_in_between!(68, 128);
impl_int_in_between!(69, 128);
impl_int_in_between!(70, 128);
impl_int_in_between!(71, 128);
impl_int_in_between!(72, 128);
impl_int_in_between!(73, 128);
impl_int_in_between!(74, 128);
impl_int_in_between!(75, 128);
impl_int_in_between!(76, 128);
impl_int_in_between!(77, 128);
impl_int_in_between!(78, 128);
impl_int_in_between!(79, 128);
impl_int_in_between!(80, 128);
impl_int_in_between!(81, 128);
impl_int_in_between!(82, 128);
impl_int_in_between!(83, 128);
impl_int_in_between!(84, 128);
impl_int_in_between!(85, 128);
impl_int_in_between!(86, 128);
impl_int_in_between!(87, 128);
impl_int_in_between!(88, 128);
impl_int_in_between!(89, 128);
impl_int_in_between!(90, 128);
impl_int_in_between!(91, 128);
impl_int_in_between!(92, 128);
impl_int_in_between!(93, 128);
impl_int_in_between!(94, 128);
impl_int_in_between!(95, 128);
impl_int_in_between!(96, 128);
impl_int_in_between!(97, 128);
impl_int_in_between!(98, 128);
impl_int_in_between!(99, 128);
impl_int_in_between!(100, 128);
impl_int_in_between!(101, 128);
impl_int_in_between!(102, 128);
impl_int_in_between!(103, 128);
impl_int_in_between!(104, 128);
impl_int_in_between!(105, 128);
impl_int_in_between!(106, 128);
impl_int_in_between!(107, 128);
impl_int_in_between!(108, 128);
impl_int_in_between!(109, 128);
impl_int_in_between!(110, 128);
impl_int_in_between!(111, 128);
impl_int_in_between!(112, 128);
impl_int_in_between!(113, 128);
impl_int_in_between!(114, 128);
impl_int_in_between!(115, 128);
impl_int_in_between!(116, 128);
impl_int_in_between!(117, 128);
impl_int_in_between!(118, 128);
impl_int_in_between!(119, 128);
impl_int_in_between!(120, 128);
impl_int_in_between!(121, 128);
impl_int_in_between!(122, 128);
impl_int_in_between!(123, 128);
impl_int_in_between!(124, 128);
impl_int_in_between!(125, 128);
impl_int_in_between!(126, 128);
impl_int_in_between!(127, 128);
*/

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::exhaust::exhaust,
        alloc::{vec, vec::Vec},
    };

    extern crate alloc;

    #[test]
    fn exhaust_u0() {
        let exhaust: Vec<_> = exhaust().map(u0::get).collect();
        assert_eq!(exhaust.len(), 1 << 0);
        assert_eq!(exhaust, vec![0]);
    }

    #[test]
    fn exhaust_i0() {
        let exhaust: Vec<_> = exhaust().map(i0::get).collect();
        assert_eq!(exhaust.len(), 1 << 0);
        assert_eq!(exhaust, vec![0]);
    }

    #[test]
    fn exhaust_u1() {
        let exhaust: Vec<_> = exhaust().map(u1::get).collect();
        assert_eq!(exhaust.len(), 1 << 1);
        assert_eq!(exhaust, vec![0, 1]);
    }

    #[test]
    fn exhaust_i1() {
        let exhaust: Vec<_> = exhaust().map(i1::get).collect();
        assert_eq!(exhaust.len(), 1 << 1);
        assert_eq!(exhaust, vec![0, -1]);
    }

    #[test]
    fn exhaust_u2() {
        let exhaust: Vec<_> = exhaust().map(u2::get).collect();
        assert_eq!(exhaust.len(), 1 << 2);
        assert_eq!(exhaust, vec![0, 1, 2, 3]);
    }

    #[test]
    fn exhaust_i2() {
        let exhaust: Vec<_> = exhaust().map(i2::get).collect();
        assert_eq!(exhaust.len(), 1 << 2);
        assert_eq!(exhaust, vec![0, 1, -1, -2]);
    }

    #[test]
    fn exhaust_u3() {
        let exhaust: Vec<_> = exhaust().map(u3::get).collect();
        assert_eq!(exhaust.len(), 1 << 3);
        assert_eq!(exhaust, vec![0, 1, 2, 3, 4, 5, 6, 7]);
    }

    #[test]
    fn exhaust_i3() {
        let exhaust: Vec<_> = exhaust().map(i3::get).collect();
        assert_eq!(exhaust.len(), 1 << 3);
        assert_eq!(exhaust, vec![0, 1, -1, 2, -2, 3, -3, -4]);
    }

    #[test]
    fn exhaust_u7() {
        let exhaust: Vec<_> = exhaust().map(u7::get).collect();
        assert_eq!(exhaust.len(), 1 << 7);
        assert_eq!(exhaust, (0..=127).collect::<Vec<_>>());
    }

    #[test]
    fn exhaust_i7() {
        let exhaust: Vec<_> = exhaust().map(i7::get).collect();
        assert_eq!(exhaust.len(), 1 << 7);
        assert_eq!(exhaust[..10], vec![0, 1, -1, 2, -2, 3, -3, 4, -4, 5]);
        assert_eq!(exhaust[120..], vec![-60, 61, -61, 62, -62, 63, -63, -64]);
    }

    #[test]
    fn exhaust_u9() {
        let exhaust: Vec<_> = exhaust().map(u9::get).collect();
        assert_eq!(exhaust.len(), 1 << 9);
        assert_eq!(exhaust, (0..=511).collect::<Vec<_>>());
    }

    #[test]
    fn exhaust_i9() {
        let exhaust: Vec<_> = exhaust().map(i9::get).collect();
        assert_eq!(exhaust.len(), 1 << 9);
        assert_eq!(exhaust[..10], vec![0, 1, -1, 2, -2, 3, -3, 4, -4, 5]);
        assert_eq!(exhaust[510..], vec![-255, -256]);
    }
}
