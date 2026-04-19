//! Implementations for int-like types.

#![expect(clippy::verbose_bit_mask, reason = "very hot loops: efficiency")]

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
        crate::reflection::type_of,
        ::malachite::{
            Natural,
            base::num::basic::traits::{One as _, Zero as _},
        },
    };

    impl Construct for Natural {
        #[inline]
        #[expect(
            clippy::needless_return,
            reason = "in case a function body is added later"
        )]
        fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
            if !visited.insert(type_of::<Self>()) {
                return;
            }
            // just in case
        }

        #[inline]
        fn type_former() -> TypeFormer<Self> {
            TypeFormer::Literal(Literal {
                deserialize: |s| s.parse().ok(),
                generate: |prng| {
                    if (prng.rand() & 3) == 0 {
                        return Self::ZERO;
                    }

                    let mut acc: Self = Self::ONE;

                    #[expect(clippy::arithmetic_side_effects, reason = "not with `malachite`")]
                    while (prng.rand() & 3) != 0 {
                        acc <<= 1_u8;
                        acc |= Self::from((prng.rand() & 1) != 0);
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
                serialize: |value: &Self| value.to_string(),
            })
        }

        #[inline]
        fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
            visit_self(self)
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

    use {super::*, crate::reflection::type_of, ::num_bigint::BigUint};

    impl Construct for BigUint {
        #[inline]
        #[expect(
            clippy::needless_return,
            reason = "in case a function body is added later"
        )]
        fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
            if !visited.insert(type_of::<Self>()) {
                return;
            }
            // just in case
        }

        #[inline]
        fn type_former() -> TypeFormer<Self> {
            TypeFormer::Literal(Literal {
                deserialize: |s| s.parse().ok(),
                generate: |prng| {
                    // Copied with small (unfortunately incompatible)
                    // modifications from `arbitrary_unsigned`.

                    if (prng.rand() & 3) == 0 {
                        return Self::ZERO;
                    }

                    let mut acc: Self = Self::from(1_u8);

                    #[expect(clippy::arithmetic_side_effects, reason = "not with `malachite`")]
                    while (prng.rand() & 3) != 0 {
                        acc <<= 1_u8;
                        acc |= Self::from((prng.rand() & 1) != 0);
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
                serialize: |value: &Self| value.to_string(),
            })
        }

        #[inline]
        fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
            visit_self(self)
        }
    }
}

use {
    crate::{
        construct::{Construct, Literal, TypeFormer, visit_self},
        reflection::{Type, type_of},
    },
    std::collections::BTreeSet,
};

/// Generate an arbitrary value for an
/// unsigned integer of fixed but unspecified width.
macro_rules! arbitrary_unsigned {
    // TODO: iterate over a `u64` as 64 booleans
    // instead of recomputing each
    ($u:ty, $prng:ident) => {{
        if ($prng.rand() & 3) == 0 {
            return 0;
        }

        let mut acc: $u = 1;

        while ($prng.rand() & 3) != 0 {
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
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| (prng.rand() & 1) != 0,
            serialize: |value: &Self| value.to_string(),
            shrink: |b| -> Box<dyn Iterator<Item = Self>> {
                Box::new(b.then_some(false).into_iter())
            },
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for u8 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_unsigned!(Self, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for u16 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_unsigned!(Self, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for u32 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_unsigned!(Self, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for u64 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_unsigned!(Self, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for u128 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_unsigned!(Self, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for usize {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_unsigned!(Self, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for i8 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_signed!(u8, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for i16 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_signed!(u16, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for i32 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_signed!(u32, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for i64 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_signed!(u64, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for i128 {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_signed!(u128, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}

impl Construct for isize {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| s.parse().ok(),
            generate: |prng| arbitrary_signed!(usize, prng),
            serialize: |value: &Self| value.to_string(),
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}
