//! Implementation for `Box<_>`.

use {
    crate::{
        construct::{
            Construct, CtorFn, IntroductionRules, ShallowConstructor, visit_self, visit_self_or,
        },
        hash::{Map, Set, empty_set},
        reflection::{_registry_mut, Type, TypeInfo, register, type_of},
    },
    core::{any::type_name, iter},
    std::sync::{Arc, OnceLock},
};

impl<T: Construct> Construct for Box<T> {
    #[inline]
    fn info() -> &'static TypeInfo {
        static CACHE: OnceLock<Arc<TypeInfo>> = OnceLock::new();
        CACHE.get_or_init(|| register::<Self>(empty_set(), &mut _registry_mut()))
    }

    #[inline]
    fn introduction_rules() -> IntroductionRules<Self> {
        IntroductionRules::Algebraic {
            constructors: vec![ShallowConstructor {
                construct: CtorFn::new(|mut terms| Box::new(terms.must_pop())),
                immediate_dependencies: iter::once(type_of::<T>()).collect(),
            }],
        }
    }

    #[inline]
    fn register_all_immediate_dependencies(
        visited: &Set<Type>,
        registry: &mut Map<Type, Arc<TypeInfo>>,
    ) {
        let id = type_of::<Self>();
        let mut visited = visited.clone();
        let not_already_visited = visited.insert(id);
        assert!(
            not_already_visited,
            "internal `pbt` error: `visited` already contained `Self = {}` (`visited` was {visited:?})",
            type_name::<Self>(),
        );
        register::<T>(visited, registry);
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self(self).chain(self.as_ref().visit_deep())
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_or(self, || self.as_ref().visit_shallow())
    }
}
