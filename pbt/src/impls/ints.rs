//! Implementations for int-like types.

use {
    crate::{
        construct::{Construct, Literal, Prng, TypeFormer, visit_self},
        hash::{Map, Set, empty_set},
        reflection::{_registry_mut, TermsOfVariousTypes, Type, TypeInfo, register},
    },
    core::num::NonZero,
    std::sync::{Arc, OnceLock},
    wyrand::WyRand,
};

/// Subtract the entire term from itself (=> 0),
/// then subtract half *less* each time thereafter:
/// e.g. for 100, this would return [0, 50, 75, 88, 94, 97, 99].
macro_rules! shrink_int {
    () => {
        |&u| -> Box<dyn Iterator<Item = Self>> {
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
        _prng: &mut Prng,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn info() -> &'static TypeInfo {
        static CACHE: OnceLock<Arc<TypeInfo>> = OnceLock::new();
        CACHE.get_or_init(|| register::<Self>(empty_set(), &mut _registry_mut()))
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
            generate: |prng| (prng.rand() & 1) != 0,
            shrink: |&b| -> Box<dyn Iterator<Item = Self>> {
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
        _prng: &mut Prng,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn info() -> &'static TypeInfo {
        static CACHE: OnceLock<Arc<TypeInfo>> = OnceLock::new();
        CACHE.get_or_init(|| register::<Self>(empty_set(), &mut _registry_mut()))
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
