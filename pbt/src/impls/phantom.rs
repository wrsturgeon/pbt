//! Implementation for `PhantomData<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            visit_self,
        },
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register},
        size::Size,
    },
    core::{marker::PhantomData, num::NonZero},
    std::collections::BTreeSet,
};

impl<T: Construct> Construct for PhantomData<T> {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut wyrand::WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<T>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
                call: CtorFn::new(|_terms| PhantomData),
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
