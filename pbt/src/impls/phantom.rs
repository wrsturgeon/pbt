//! Implementation for `PhantomData<_>`.

use {
    crate::{
        multiset::Multiset,
        pbt::{
            Algebraic, ArbitraryFn, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt,
            TypeFormer, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    alloc::collections::BTreeSet,
    core::{marker::PhantomData, num::NonZero},
};

impl<T: Pbt> Pbt for PhantomData<T> {
    #[inline]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        sccs: &mut StronglyConnectedComponents,
    ) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<T>(visited.clone(), sccs);
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
                arbitrary: ArbitraryFn::new(|_, _| Ok(Some(PhantomData))),
                call: CtorFn::new(|_terms| Some(PhantomData)),
                immediate_dependencies: Multiset::new(),
            }],
            elimination_rule: ElimFn::new(|_| Decomposition {
                ctor_idx: const { NonZero::new(1).unwrap() },
                fields: TermsOfVariousTypes::new(),
            }),
        })
    }

    #[inline]
    fn visit_deep<V>(&self) -> impl Iterator<Item = V>
    where
        V: Pbt,
    {
        visit_self(self)
    }
}
