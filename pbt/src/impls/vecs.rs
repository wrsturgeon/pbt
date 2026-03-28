//! Implementation for `Vec<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            visit_self, visit_self_or,
        },
        hash::Set,
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        size::Size,
    },
    core::{any::type_name, num::NonZero},
};

impl<T: Construct> Construct for Vec<T> {
    #[inline]
    fn arbitrary_fields_for_ctor(
        ctor_idx: NonZero<usize>,
        prng: &mut wyrand::WyRand,
        size: Size,
    ) -> TermsOfVariousTypes {
        let mut sizes = size.partition::<Self>(ctor_idx, prng);
        let mut fields = TermsOfVariousTypes::new();
        match ctor_idx.get() {
            1 => {}
            2 => {
                let () = fields.push(sizes.arbitrary::<T>(prng));
                let () = fields.push(sizes.arbitrary::<Self>(prng));
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
                    call: CtorFn::new(|_| vec![]),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    call: CtorFn::new(|terms| {
                        let mut acc = terms.must_pop::<Self>(); // tail
                        acc.push(terms.must_pop::<T>()); // head
                        acc
                    }),
                    immediate_dependencies: [type_of::<T>(), type_of::<Self>()]
                        .into_iter()
                        .collect(),
                },
            ],
            elimination_rule: ElimFn::new(|mut v| {
                let mut fields = TermsOfVariousTypes::new();
                let ctor_idx = if let Some(head) = v.pop() {
                    let () = fields.push::<T>(head);
                    let () = fields.push::<Self>(v);
                    2
                } else {
                    1
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
        // TODO: Can't visit all the recursive tails of this
        // vec viewed as a Haskell-style linked list, since
        // references can't escape into an iterator --
        // at least not without `collect`ing and fucking all efficiency.
        visit_self(self).chain(self.iter().flat_map(T::visit_deep))
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_or(self, || self.iter().flat_map(T::visit_shallow))
    }
}
