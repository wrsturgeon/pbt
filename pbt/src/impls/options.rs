//! Implementation for `Option<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            push_arbitrary_field, visit_self,
        },
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    core::{iter, num::NonZero},
    std::collections::BTreeSet,
};

impl<T: Construct> Construct for Option<T> {
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
            introduction_rules: vec![
                IntroductionRule {
                    arbitrary_fields: |_, _| Ok(TermsOfVariousTypes::new()),
                    call: CtorFn::new(|_| Some(None)),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    arbitrary_fields: |prng, mut sizes| {
                        let mut fields = TermsOfVariousTypes::new();
                        push_arbitrary_field::<T>(&mut fields, &mut sizes, prng)?;
                        Ok(fields)
                    },
                    call: CtorFn::new(|terms| Some(Some(terms.must_pop()))),
                    immediate_dependencies: iter::once(type_of::<T>()).collect(),
                },
            ],
            elimination_rule: ElimFn::new(|opt| {
                let mut fields = TermsOfVariousTypes::new();
                let ctor_idx = match opt {
                    None => 1,
                    Some(t) => {
                        let () = fields.push::<T>(t);
                        2
                    }
                };
                Decomposition {
                    // SAFETY: Case analysis above.
                    ctor_idx: unsafe { NonZero::new_unchecked(ctor_idx) },
                    fields,
                }
            }),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(self.as_ref().map(T::visit_deep).into_iter().flatten())
    }
}
