//! Implementation for `Vec<_>`.

use {
    crate::{
        pbt::{
            Algebraic, ArbitraryFn, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt,
            TypeFormer, arbitrary_field, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    alloc::collections::BTreeSet,
    core::num::NonZero,
};

impl<Lhs: Pbt, Rhs: Pbt> Pbt for (Lhs, Rhs) {
    #[inline]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        sccs: &mut StronglyConnectedComponents,
    ) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<Lhs>(visited.clone(), sccs);
        let () = register::<Rhs>(visited.clone(), sccs);
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            elimination_rule: ElimFn::new(|(lhs, rhs)| {
                let mut fields = TermsOfVariousTypes::new();
                let () = fields.push(lhs);
                let () = fields.push(rhs);
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
            introduction_rules: vec![IntroductionRule {
                arbitrary: ArbitraryFn::new(|prng, mut sizes| {
                    let lhs = arbitrary_field::<Lhs>(&mut sizes, prng)?;
                    let rhs = arbitrary_field::<Rhs>(&mut sizes, prng)?;
                    Ok(Some((lhs, rhs)))
                }),
                call: CtorFn::new(|fields| {
                    let rhs: Rhs = fields.must_pop();
                    let lhs: Lhs = fields.must_pop();
                    Some((lhs, rhs))
                }),
                immediate_dependencies: [type_of::<Lhs>(), type_of::<Rhs>()].into_iter().collect(),
            }],
        })
    }

    #[inline]
    fn visit_deep<V>(&self) -> impl Iterator<Item = V>
    where
        V: Pbt,
    {
        let (ref lhs, ref rhs) = *self;
        visit_self(self)
            .chain(lhs.visit_deep())
            .chain(rhs.visit_deep())
    }
}
