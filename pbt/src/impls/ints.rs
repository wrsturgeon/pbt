//! Implementations for int-like types.

use {
    crate::{
        construct::{Construct, Generate, Prng, ShallowConstructor, visit_self},
        hash::{Map, Set, empty_set},
        multiset::Multiset,
        reflection::{_registry_mut, Type, TypeInfo, register},
    },
    std::sync::{Arc, OnceLock},
};

impl Construct for bool {
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
    }

    #[inline]
    fn shallow_constructors() -> Vec<ShallowConstructor<Self>> {
        vec![ShallowConstructor {
            #[expect(
                clippy::as_conversions,
                reason = "Stateless function from the same types to same type."
            )]
            construct: Generate::new(
                (|prng| (prng.u64() & 1) != 0) as for<'prng> fn(&'prng mut Prng) -> Self,
            ),
            immediate_dependencies: Multiset::new(),
        }]
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
    fn info() -> &'static TypeInfo {
        static CACHE: OnceLock<Arc<TypeInfo>> = OnceLock::new();
        CACHE.get_or_init(|| register::<Self>(empty_set(), &mut _registry_mut()))
    }

    #[inline]
    fn register_all_immediate_dependencies(
        _visited: &Set<Type>,
        _registry: &mut Map<Type, Arc<TypeInfo>>,
    ) {
    }

    #[inline]
    fn shallow_constructors() -> Vec<ShallowConstructor<Self>> {
        vec![ShallowConstructor {
            construct: Generate::new(Prng::u64),
            immediate_dependencies: Multiset::new(),
        }]
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
