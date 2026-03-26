//! Implementations for int-like types.

use {
    crate::{
        construct::{Construct, IntroductionRules, visit_self},
        hash::{Map, Set, empty_set},
        reflection::{_registry_mut, Type, TypeInfo, register},
    },
    std::sync::{Arc, OnceLock},
    wyrand::WyRand,
};

impl Construct for bool {
    #[inline]
    fn info() -> &'static TypeInfo {
        static CACHE: OnceLock<Arc<TypeInfo>> = OnceLock::new();
        CACHE.get_or_init(|| register::<Self>(empty_set(), &mut _registry_mut()))
    }

    #[inline]
    fn introduction_rules() -> IntroductionRules<Self> {
        IntroductionRules::Literal {
            generate: |prng| (prng.rand() & 1) != 0,
        }
    }

    #[inline]
    fn register_all_immediate_dependencies(
        _visited: &Set<Type>,
        _registry: &mut Map<Type, Arc<TypeInfo>>,
    ) {
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
    fn introduction_rules() -> IntroductionRules<Self> {
        IntroductionRules::Literal {
            generate: WyRand::rand,
        }
    }

    #[inline]
    fn register_all_immediate_dependencies(
        _visited: &Set<Type>,
        _registry: &mut Map<Type, Arc<TypeInfo>>,
    ) {
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
