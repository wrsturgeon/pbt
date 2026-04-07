//! Implementation for `Vec<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
    },
    core::num::NonZero,
    std::collections::BTreeSet,
};

impl<Lhs: Construct, Rhs: Construct> Construct for (Lhs, Rhs) {
    #[inline]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<Lhs>(visited.clone());
        let () = register::<Rhs>(visited.clone());
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
                arbitrary_fields: |prng, mut sizes| {
                    let mut fields = TermsOfVariousTypes::new();
                    fields.push(sizes.arbitrary::<Lhs>(prng));
                    fields.push(sizes.arbitrary::<Rhs>(prng));
                    fields
                },
                call: CtorFn::new(|fields| (fields.must_pop(), fields.must_pop())),
                immediate_dependencies: [type_of::<Lhs>(), type_of::<Rhs>()].into_iter().collect(),
            }],
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        let (ref lhs, ref rhs) = *self;
        visit_self(self)
            .chain(lhs.visit_deep())
            .chain(rhs.visit_deep())
    }
}
