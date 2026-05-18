//! Implementation for `()`.

use {
    crate::{
        StronglyConnectedComponents,
        multiset::Multiset,
        pbt::{
            Algebraic, ArbitraryFn, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt,
            TypeFormer, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, type_of},
    },
    alloc::collections::BTreeSet,
    core::num::NonZero,
};

impl Pbt for () {
    #[inline]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        _sccs: &mut StronglyConnectedComponents,
    ) {
        let _dup: bool = visited.insert(type_of::<Self>());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            elimination_rule: ElimFn::new(|()| Decomposition {
                ctor_idx: const { NonZero::new(1).unwrap() },
                fields: TermsOfVariousTypes::new(),
            }),
            introduction_rules: vec![IntroductionRule {
                arbitrary: ArbitraryFn::new(|_, _| Ok(Some(()))),
                call: CtorFn::new(|_terms| Some(())),
                immediate_dependencies: Multiset::new(),
            }],
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
