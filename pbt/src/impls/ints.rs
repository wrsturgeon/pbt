//! Implementations for int-like types.

#[cfg(feature = "malachite")]
mod malachite {
    #![allow(
        clippy::allow_attributes,
        clippy::wildcard_imports,
        reason = "the purpose of this effectively transparent module is only feature-gating"
    )]

    //! Implementations for numeric types from the `malachite` crate.

    use {
        super::*,
        ::malachite::{
            Natural,
            base::num::basic::traits::{One as _, Zero as _},
        },
    };

    impl Construct for Natural {
        #[inline]
        fn arbitrary_fields_for_ctor(
            _ctor_idx: NonZero<usize>,
            _prng: &mut WyRand,
            _size: Size,
        ) -> TermsOfVariousTypes {
            TermsOfVariousTypes::new()
        }

        #[inline]
        fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

        #[inline]
        fn type_former() -> TypeFormer<Self> {
            TypeFormer::Literal(Literal {
                generate: |prng| {
                    // Copied with small (unfortunately incompatible)
                    // modifications from `arbitrary_unsigned` above.

                    let mut one_in_n = const { NonZero::new(4_u64).unwrap() };

                    if (prng.rand() % one_in_n) == 0 {
                        return Self::ZERO;
                    }
                    one_in_n = one_in_n.saturating_add(1);

                    let mut acc: Self = Self::ONE;

                    #[expect(clippy::arithmetic_side_effects, reason = "not with `malachite`")]
                    while (prng.rand() % one_in_n) != 0 {
                        acc <<= 1_u8;
                        acc |= Self::from((prng.rand() & 1) != 0);

                        one_in_n = one_in_n.saturating_add(1);
                    }
                    acc
                },
                shrink: |u| -> Box<dyn Iterator<Item = Self>> {
                    // Copied with small (unfortunately incompatible)
                    // modifications from `shrink_int` above.

                    Box::new((0_usize..).map_while(move |shr| {
                        #[expect(clippy::arithmetic_side_effects, reason = "not with `malachite`")]
                        let subtrahend = &u >> shr;
                        #[allow(
                            clippy::allow_attributes,
                            clippy::default_numeric_fallback,
                            reason = "type varies"
                        )]
                        #[expect(
                            clippy::arithmetic_side_effects,
                            reason = "`u >> _` is always <= `u`"
                        )]
                        (subtrahend != 0).then(|| &u - subtrahend)
                    }))
                },
            })
        }

        #[inline]
        fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
            visit_self(self)
        }

        #[inline]
        fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
            visit_self_opt(self).into_iter()
        }
    }
}

#[cfg(feature = "num-bigint")]
mod num_bigint {
    #![allow(
        clippy::allow_attributes,
        clippy::wildcard_imports,
        reason = "the purpose of this effectively transparent module is only feature-gating"
    )]

    //! Implementations for numeric types from the `num_bigint` crate.

    use {super::*, ::num_bigint::BigUint};

    impl Construct for BigUint {
        #[inline]
        fn arbitrary_fields_for_ctor(
            _ctor_idx: NonZero<usize>,
            _prng: &mut WyRand,
            _size: Size,
        ) -> TermsOfVariousTypes {
            TermsOfVariousTypes::new()
        }

        #[inline]
        fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

        #[inline]
        fn type_former() -> TypeFormer<Self> {
            TypeFormer::Literal(Literal {
                generate: |prng| {
                    // Copied with small (unfortunately incompatible)
                    // modifications from `arbitrary_unsigned` above.

                    let mut one_in_n = const { NonZero::new(4_u64).unwrap() };

                    if (prng.rand() % one_in_n) == 0 {
                        return Self::ZERO;
                    }
                    one_in_n = one_in_n.saturating_add(1);

                    let mut acc: Self = Self::from(1_u8);

                    #[expect(clippy::arithmetic_side_effects, reason = "not with `malachite`")]
                    while (prng.rand() % one_in_n) != 0 {
                        acc <<= 1_u8;
                        acc |= Self::from((prng.rand() & 1) != 0);

                        one_in_n = one_in_n.saturating_add(1);
                    }
                    acc
                },
                shrink: |u| -> Box<dyn Iterator<Item = Self>> {
                    // Copied with small (unfortunately incompatible)
                    // modifications from `shrink_int` above.

                    Box::new((0_usize..).map_while(move |shr| {
                        #[expect(clippy::arithmetic_side_effects, reason = "not with `malachite`")]
                        let subtrahend = &u >> shr;
                        #[allow(
                            clippy::allow_attributes,
                            clippy::default_numeric_fallback,
                            reason = "type varies"
                        )]
                        #[expect(
                            clippy::arithmetic_side_effects,
                            reason = "`u >> _` is always <= `u`"
                        )]
                        (subtrahend != Self::ZERO).then(|| &u - subtrahend)
                    }))
                },
            })
        }

        #[inline]
        fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
            visit_self(self)
        }

        #[inline]
        fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
            visit_self_opt(self).into_iter()
        }
    }
}

use {
    crate::{
        construct::{Construct, Literal, TypeFormer, visit_self, visit_self_opt},
        reflection::{TermsOfVariousTypes, Type},
        size::Size,
    },
    core::num::NonZero,
    std::collections::BTreeSet,
    wyrand::WyRand,
};

/// Generate an arbitrary value for an
/// unsigned integer of fixed but unspecified width.
macro_rules! arbitrary_unsigned {
    // TODO: iterate over a `u64` as 64 booleans
    // instead of recomputing each
    ($u:ty, $prng:ident) => {{
        // Larger bit widths should be allowed to generate
        // huge values, whereas all integers should
        // generate relatively small numbers relatively often,
        // so this probability denominator increments each round.
        let mut one_in_n = const { NonZero::new(4_u64).unwrap() };

        if ($prng.rand() % one_in_n) == 0 {
            return 0;
        }
        one_in_n = one_in_n.saturating_add(1);

        let mut acc: $u = 1;

        while ($prng.rand() % one_in_n) != 0 {
            #[allow(
                clippy::allow_attributes,
                clippy::default_numeric_fallback,
                reason = "type varies"
            )]
            if acc.cast_signed() < 0 {
                acc = <$u>::MAX;
                break;
            }
            acc <<= 1_u8;
            acc |= <$u>::from(($prng.rand() & 1) != 0);

            one_in_n = one_in_n.saturating_add(1);
        }
        acc
    }};
}

/// Generate an arbitrary value for a
/// signed integer of fixed but unspecified width.
macro_rules! arbitrary_signed {
    ($u:ty, $prng:ident) => {{
        let unsigned = arbitrary_unsigned!($u, $prng);
        if ($prng.rand() & 1) == 0 {
            unsigned.cast_signed()
        } else {
            (!unsigned).cast_signed()
        }
    }};
}

/// Subtract the entire term from itself (=> 0),
/// then subtract half *less* each time thereafter:
/// e.g. for 100, this would return [0, 50, 75, 88, 94, 97, 99].
macro_rules! shrink_int {
    () => {
        |u| -> Box<dyn Iterator<Item = Self>> {
            Box::new((0..).map_while(move |shr| {
                let subtrahend = u.checked_shr(shr)?;
                #[allow(
                    clippy::allow_attributes,
                    clippy::default_numeric_fallback,
                    reason = "type varies"
                )]
                #[expect(clippy::arithmetic_side_effects, reason = "`u >> _` is always <= `u`")]
                (subtrahend != 0).then(|| u - subtrahend)
            }))
        }
    };
}

impl Construct for bool {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {
        // n/a
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| (prng.rand() & 1) != 0,
            shrink: |b| -> Box<dyn Iterator<Item = Self>> {
                Box::new(b.then_some(false).into_iter())
            },
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for u8 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_unsigned!(Self, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for u16 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_unsigned!(Self, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for u32 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_unsigned!(Self, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for u64 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_unsigned!(Self, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for u128 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_unsigned!(Self, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for usize {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_unsigned!(Self, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for i8 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_signed!(u8, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for i16 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_signed!(u16, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for i32 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_signed!(u32, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for i64 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_signed!(u64, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for i128 {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_signed!(u128, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}

impl Construct for isize {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| arbitrary_signed!(usize, prng),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_opt(self).into_iter()
    }
}
