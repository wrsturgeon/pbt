//! Implementations for `std::rc::Rc<_>` and `std::sync::Arc<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
    },
    core::{iter, num::NonZero},
    std::{collections::BTreeSet, rc::Rc, sync::Arc},
};

impl<T: Construct> Construct for Rc<T> {
    #[inline]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<T>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
                arbitrary_fields: |prng, mut sizes| {
                    let mut fields = TermsOfVariousTypes::new();
                    fields.push(sizes.arbitrary::<T>(prng));
                    fields
                },
                call: CtorFn::new(|terms| Rc::new(terms.must_pop())),
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
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(self.as_ref().visit_deep())
    }
}

impl<T: Construct> Construct for Arc<T> {
    #[inline]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<T>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
                arbitrary_fields: |prng, mut sizes| {
                    let mut fields = TermsOfVariousTypes::new();
                    fields.push(sizes.arbitrary::<T>(prng));
                    fields
                },
                call: CtorFn::new(|terms| Arc::new(terms.must_pop())),
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
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(self.as_ref().visit_deep())
    }
}
