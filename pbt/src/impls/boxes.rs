//! Implementation for `Box<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            push_arbitrary_field, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    core::{iter, num::NonZero},
    std::collections::BTreeSet,
};

impl<T: Construct> Construct for Box<T> {
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
                arbitrary_fields: |prng, mut sizes| {
                    let mut fields = TermsOfVariousTypes::new();
                    push_arbitrary_field::<T>(&mut fields, &mut sizes, prng)?;
                    Ok(fields)
                },
                call: CtorFn::new(|terms| Some(Box::new(terms.must_pop()))),
                immediate_dependencies: iter::once(type_of::<T>()).collect(),
            }],
            elimination_rule: ElimFn::new(|boxed| {
                let mut fields = TermsOfVariousTypes::new();
                let () = fields.push::<T>(*boxed);
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(self.as_ref().visit_deep())
    }
}
