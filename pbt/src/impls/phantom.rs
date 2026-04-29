//! Implementation for `PhantomData<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            visit_self,
        },
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    core::{marker::PhantomData, num::NonZero},
    std::collections::BTreeSet,
};

impl<T: Construct> Construct for PhantomData<T> {
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
                arbitrary_fields: |_, _| Ok(TermsOfVariousTypes::new()),
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
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }
}
