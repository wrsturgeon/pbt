//! Implementations for int-like types.

use {
    crate::{
        construct::{Construct, Literal, TypeFormer, visit_self},
        hash::{Map, Set},
        reflection::{TermsOfVariousTypes, Type, TypeInfo},
        size::Size,
    },
    core::num::NonZero,
    std::sync::Arc,
    wyrand::WyRand,
};

/// The corner cases of a signed fixed-width integer type.
macro_rules! int_corners {
    ($ty:ty) => {
        [0, 1, -1, <$ty>::MAX, <$ty>::MIN]
    };
}

/// Subtract the entire term from itself (=> 0),
/// then subtract half *less* each time thereafter:
/// e.g. for 100, this would return [0, 50, 75, 88, 94, 97, 99].
macro_rules! shrink_int {
    () => {
        |u| -> Box<dyn Iterator<Item = Self>> {
            Box::new((0_u16..).map_while(move |shr| {
                let subtrahend = u >> shr;
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
    fn register_all_immediate_dependencies(
        _visited: &Set<Type>,
        _registry: &mut Map<Type, Arc<TypeInfo>>,
    ) {
        // n/a
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            corners: vec![false, true],
            generate: |prng| (prng.rand() & 1) != 0,
            shrink: |b| -> Box<dyn Iterator<Item = Self>> {
                Box::new(b.then_some(false).into_iter())
            },
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self(self)
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
    fn register_all_immediate_dependencies(
        _visited: &Set<Type>,
        _registry: &mut Map<Type, Arc<TypeInfo>>,
    ) {
        // n/a
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            corners: int_corners!(i64)
                .into_iter()
                .map(i64::cast_unsigned)
                .collect(),
            generate: WyRand::rand,
            shrink: shrink_int!(),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self(self)
    }
}
