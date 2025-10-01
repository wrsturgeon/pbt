use {
    crate::{
        error,
        iter::RemoveDuplicates,
        lower_bits::LowerBits,
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        traits::{
            corner::Corner, decimate::Decimate, refine::Refine, rnd::Rnd, size::Size,
            weight::Weight,
        },
    },
    core::iter,
    rand_core::RngCore,
};

/// Implement core traits for an unsigned integer type.
macro_rules! impl_unsigned {
    ($ty:ident) => {
        impl From<$ty> for LowerBits {
            #[inline]
            fn from(mut value: $ty) -> Self {
                let maybe_bits = (value != 0).then(|| {
                    let mut acc = vec![];
                    while value != 1 {
                        let () = acc.push((value & 1) != 0);
                        value >>= 1_u32;
                    }
                    acc
                });
                Self { maybe_bits }
            }
        }

        impl TryFrom<LowerBits> for $ty {
            type Error = error::Overflow;
            #[inline]
            fn try_from(LowerBits { maybe_bits }: LowerBits) -> Result<Self, Self::Error> {
                Ok(match maybe_bits {
                    None => 0,
                    Some(bits) => {
                        let mut acc: Self = 1;
                        for bit in bits {
                            if (acc & const { 1 << (Self::BITS - 1) }) != 0 {
                                return Err(error::Overflow);
                            }
                            // acc = acc.unchecked_shl(1); // TODO: when it's mainlined (if ever)
                            acc = (acc << 1) | Self::from(bit);
                        }
                        acc
                    }
                })
            }
        }

        impl LowerBits {
            /// Unsafely convert this value into the eponymous type.
            /// # Safety
            /// `self` must not represent a value that would overflow that type.
            #[inline]
            pub unsafe fn $ty(&self) -> $ty {
                let Self { ref maybe_bits } = *self;
                match *maybe_bits {
                    None => 0,
                    Some(ref bits) => {
                        let mut acc: $ty = 1;
                        for &bit in bits {
                            #[cfg(all(test, debug_assertions))]
                            {
                                assert_eq!((acc & const { 1 << ($ty::BITS - 1) }), 0);
                            }
                            // acc = acc.unchecked_shl(1); // TODO: when it's mainlined (if ever)
                            acc = (acc << 1) | $ty::from(bit);
                        }
                        acc
                    }
                }
            }
        }

        impl Weight for $ty {
            const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> =
                MaybeInstantiable::Instantiable(MaybeInfinite::Finite(
                    // TODO: THIS SHOULD FAIL!
                    0.,
                ));
            const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> =
                MaybeInstantiable::Instantiable(MaybeInfinite::Finite(
                    // TODO: THIS SHOULD FAIL!
                    0,
                ));
            #[inline]
            fn weight(&self) -> usize {
                LowerBits::from(*self).weight()
            }
        }

        impl Size for $ty {
            const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> =
                MaybeInstantiable::Instantiable(MaybeInfinite::Finite(
                    #[allow(
                        clippy::allow_attributes,
                        clippy::as_conversions,
                        clippy::cast_possible_truncation,
                        reason = "Checked below."
                    )]
                    {
                        let coerced = $ty::MAX as usize;
                        // Roundtrip test, since `TryFrom` is not (yet?) `const`:
                        if (coerced as $ty) == $ty::MAX {
                            MaybeOverflow::Contained(coerced)
                        } else {
                            MaybeOverflow::Overflow
                        }
                    },
                ));
            #[inline]
            fn size(&self) -> MaybeOverflow<usize> {
                MaybeOverflow::Contained(0)
            }
        }

        impl Corner for $ty {
            type Corners = core::array::IntoIter<Self, 6>;

            #[inline]
            fn corners() -> Self::Corners {
                [
                    0,
                    1,
                    (Self::MAX >> 1) - 1,
                    Self::MAX >> 1,
                    Self::MAX - 1,
                    Self::MAX,
                ]
                .into_iter()
            }
        }

        impl Rnd for $ty {
            #[inline]
            fn rnd<Rng: RngCore>(rng: &mut Rng, _expected_weight: f32) -> MaybeInstantiable<Self> {
                #[allow(
                    arithmetic_overflow,
                    clippy::allow_attributes,
                    clippy::arithmetic_side_effects,
                    clippy::as_conversions,
                    clippy::cast_lossless,
                    clippy::cast_possible_truncation,
                    reason = "Not possible, since bit width is taken into account."
                )]
                MaybeInstantiable::Instantiable(if const { Self::BITS > 64 } {
                    let mut acc: Self = 0;
                    let mut bits = 0_u32;
                    while bits < Self::BITS {
                        acc = (acc << 64) | (rng.next_u64() as Self);
                        bits += 64;
                    }
                    acc
                } else {
                    rng.next_u64() as Self
                })
            }
        }

        impl Decimate for $ty {
            type Decimate = iter::Map<
                RemoveDuplicates<<LowerBits as Decimate>::Decimate>,
                fn(LowerBits) -> Self,
            >;
            #[inline]
            fn decimate(&self, weight: usize) -> Self::Decimate {
                RemoveDuplicates::new(LowerBits::from(*self).decimate(weight)).map(
                    #[expect(
                        clippy::as_conversions,
                        reason = "Function pointer conversions are checked more thoroughly"
                    )]
                    {
                        (|bits: LowerBits| {
                            // SAFETY: Decimation produces values smaller than the original,
                            // and the original can't have overflowed its own type.
                            unsafe { bits.$ty() }
                        }) as fn(_) -> _
                    },
                )
            }
        }

        impl Refine for $ty {
            type Refine = iter::Map<<LowerBits as Refine>::Refine, fn(LowerBits) -> Self>;
            #[inline]
            fn refine(&self, size: usize) -> Self::Refine {
                LowerBits::from(*self).refine(size).map(
                    #[expect(
                        clippy::as_conversions,
                        reason = "Function pointer conversions are checked more thoroughly"
                    )]
                    {
                        (|bits: LowerBits| {
                            // SAFETY: Decimation produces values smaller than the original,
                            // and the original can't have overflowed its own type.
                            unsafe { bits.$ty() }
                        }) as fn(_) -> _
                    },
                )
            }
        }
    };
}

impl_unsigned!(u8);
impl_unsigned!(u16);
impl_unsigned!(u32);
impl_unsigned!(u64);
impl_unsigned!(u128);
impl_unsigned!(usize);

#[cfg(test)]
mod test {
    use {super::*, crate::impl_tests};

    impl_tests!(u8, u8);
    impl_tests!(u16, u16);
    impl_tests!(u32, u32);
    impl_tests!(u64, u64);
    impl_tests!(u128, u128);
    impl_tests!(usize, usize);
    // TODO:
    // impl_tests!(i8, i8);
    // impl_tests!(i16, i16);
    // impl_tests!(i32, i32);
    // impl_tests!(i64, i64);
    // impl_tests!(i128, i128);
    // impl_tests!(isize, isize);

    #[test]
    fn max_size_u8() {
        assert_eq!(
            u8::MAX_SIZE,
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(0x_FF)))
        );
    }

    #[test]
    fn max_size_u16() {
        assert_eq!(
            u16::MAX_SIZE,
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(
                0x_FF_FF
            )))
        );
    }

    #[test]
    fn max_size_u32() {
        assert_eq!(
            u32::MAX_SIZE,
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(
                0x_FFFF_FFFF
            )))
        );
    }

    #[test]
    fn u8_lower_bits_roundtrip() {
        for orig in u8::MIN..=u8::MAX {
            let lb = LowerBits::from(orig);
            let roundtrip = u8::try_from(lb.clone());
            assert_eq!(
                roundtrip,
                Ok(orig),
                "{orig:?} -> {lb:?} -> {roundtrip:?} =/= {orig:?}",
            );
        }
    }

    #[test]
    fn u8_size_is_self() {
        for orig in u8::MIN..=u8::MAX {
            assert_eq!(orig.size(), MaybeOverflow::Contained(usize::from(orig)));
        }
    }
}
