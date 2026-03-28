//! Implementation for `Option<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            arbitrary, visit_self, visit_self_or,
        },
        hash::Set,
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        size::Size,
    },
    core::{any::type_name, iter, num::NonZero},
};

impl<T: Construct> Construct for Option<T> {
    #[inline]
    fn arbitrary_fields_for_ctor(
        ctor_idx: NonZero<usize>,
        prng: &mut wyrand::WyRand,
        size: Size,
    ) -> TermsOfVariousTypes {
        let mut fields = TermsOfVariousTypes::new();
        match ctor_idx.get() {
            1 => {}
            2 => {
                #[expect(clippy::panic, reason = "internal invariant violated")]
                let Some(some) = arbitrary::<T>(prng, size) else {
                    panic!(
                        "uninstantiable type `{}` in constructor #{ctor_idx} of `{}`",
                        type_name::<T>(),
                        type_name::<Self>(),
                    )
                };
                let () = fields.push(some);
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
    fn register_all_immediate_dependencies(visited: &Set<Type>) {
        let id = type_of::<Self>();
        let mut visited = visited.clone();
        let not_already_visited = visited.insert(id);
        assert!(
            not_already_visited,
            "internal `pbt` error: `visited` already contained `Self = {}` (`visited` was {visited:?})",
            type_name::<Self>(),
        );
        register::<T>(visited);
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![
                IntroductionRule {
                    call: CtorFn::new(|_| None),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    call: CtorFn::new(|terms| Some(terms.must_pop())),
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
                    // SAFETY: 1 != 0
                    ctor_idx: unsafe { NonZero::new_unchecked(ctor_idx) },
                    fields,
                }
            }),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self(self).chain(self.as_ref().map(T::visit_deep).into_iter().flatten())
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_or(self, || {
            self.as_ref().map(T::visit_shallow).into_iter().flatten()
        })
    }
}
