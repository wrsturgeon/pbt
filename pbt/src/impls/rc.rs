//! Implementations for `std::rc::Rc<_>` and `std::sync::Arc<_>`.

use {
    crate::{
        pbt::{
            Algebraic, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt, TypeFormer,
            push_arbitrary_field, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    core::{iter, num::NonZero},
    std::{collections::BTreeSet, rc::Rc, sync::Arc},
};

impl<T: Pbt> Pbt for Rc<T> {
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
                call: CtorFn::new(|terms| Some(Rc::new(terms.must_pop()))),
                immediate_dependencies: iter::once(type_of::<T>()).collect(),
            }],
            elimination_rule: ElimFn::new(|rc| {
                let mut fields = TermsOfVariousTypes::new();
                let () =
                    fields.push::<T>(Rc::try_unwrap(rc).unwrap_or_else(|rc| rc.as_ref().clone()));
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
        })
    }

    #[inline]
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(self.as_ref().visit_deep())
    }
}

impl<T: Pbt> Pbt for Arc<T> {
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
                call: CtorFn::new(|terms| Some(Arc::new(terms.must_pop()))),
                immediate_dependencies: iter::once(type_of::<T>()).collect(),
            }],
            elimination_rule: ElimFn::new(|arc| {
                let mut fields = TermsOfVariousTypes::new();
                let () = fields
                    .push::<T>(Arc::try_unwrap(arc).unwrap_or_else(|arc| arc.as_ref().clone()));
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
        })
    }

    #[inline]
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(self.as_ref().visit_deep())
    }
}
