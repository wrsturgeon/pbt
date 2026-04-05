//! Implementations for `std::rc::Rc<_>` and `std::sync::Arc<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            arbitrary, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        size::Size,
    },
    core::{any::type_name, iter, num::NonZero},
    std::{collections::BTreeSet, rc::Rc, sync::Arc},
};

impl<T: Construct> Construct for Rc<T> {
    #[inline]
    fn arbitrary_fields_for_ctor(
        ctor_idx: NonZero<usize>,
        prng: &mut wyrand::WyRand,
        size: Size,
    ) -> TermsOfVariousTypes {
        let mut fields = TermsOfVariousTypes::new();
        match ctor_idx.get() {
            1 => {
                #[expect(clippy::panic, reason = "internal invariant violated")]
                let Some(unboxed) = arbitrary::<T>(prng, size) else {
                    panic!(
                        "uninstantiable type `{}` in constructor #{ctor_idx} of `{}`",
                        type_name::<T>(),
                        type_name::<Self>(),
                    )
                };
                let () = fields.push(unboxed);
            }
            #[expect(clippy::panic, reason = "internal invariant violated")]
            _ => panic!(
                "internal `pbt` error: unknown `{}` constructor index #{ctor_idx}",
                type_name::<Self>(),
            ),
        }
        fields
    }

    #[inline]
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<T>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
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
    fn arbitrary_fields_for_ctor(
        ctor_idx: NonZero<usize>,
        prng: &mut wyrand::WyRand,
        size: Size,
    ) -> TermsOfVariousTypes {
        let mut fields = TermsOfVariousTypes::new();
        match ctor_idx.get() {
            1 => {
                #[expect(clippy::panic, reason = "internal invariant violated")]
                let Some(unboxed) = arbitrary::<T>(prng, size) else {
                    panic!(
                        "uninstantiable type `{}` in constructor #{ctor_idx} of `{}`",
                        type_name::<T>(),
                        type_name::<Self>(),
                    )
                };
                let () = fields.push(unboxed);
            }
            #[expect(clippy::panic, reason = "internal invariant violated")]
            _ => panic!(
                "internal `pbt` error: unknown `{}` constructor index #{ctor_idx}",
                type_name::<Self>(),
            ),
        }
        fields
    }

    #[inline]
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<T>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
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
