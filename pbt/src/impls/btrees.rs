//! Implementation for `BTree{Map, Set}`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            visit_self, visit_self_opt,
        },
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        size::Size,
    },
    core::{any::type_name, iter, num::NonZero},
    std::collections::{BTreeMap, BTreeSet},
};

impl<T: Construct + Ord> Construct for BTreeSet<T> {
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
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<T>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![
                IntroductionRule {
                    call: CtorFn::new(|_| BTreeSet::new()),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    call: CtorFn::new(|terms| {
                        let mut acc = terms.must_pop::<Self>(); // tail
                        acc.insert(terms.must_pop::<T>()); // head
                        acc
                    }),
                    immediate_dependencies: [type_of::<T>(), type_of::<Self>()]
                        .into_iter()
                        .collect(),
                },
            ],
            elimination_rule: ElimFn::new(|mut b| {
                let mut fields = TermsOfVariousTypes::new();
                let ctor_idx = if let Some(head) = b.pop_last() {
                    let () = fields.push::<T>(head);
                    let () = fields.push::<Self>(b);
                    2
                } else {
                    1
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
        visit_self::<V, Self>(self)
            .chain(self.iter().flat_map(T::visit_deep))
            .chain({
                let mut b = self.clone();
                iter::from_fn(move || {
                    let _: T = b.pop_last()?;
                    visit_self_opt::<V, Self>(&b).cloned()
                })
            })
    }
}

impl<K: Construct + Ord, V: Construct> Construct for BTreeMap<K, V> {
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
                let () = fields.push(sizes.arbitrary::<K>(prng));
                let () = fields.push(sizes.arbitrary::<V>(prng));
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
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<K>(visited.clone());
        let () = register::<V>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![
                IntroductionRule {
                    call: CtorFn::new(|_| BTreeMap::new()),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    call: CtorFn::new(|terms| {
                        let mut acc = terms.must_pop::<Self>();
                        acc.insert(terms.must_pop::<K>(), terms.must_pop::<V>());
                        acc
                    }),
                    immediate_dependencies: [type_of::<K>(), type_of::<V>(), type_of::<Self>()]
                        .into_iter()
                        .collect(),
                },
            ],
            elimination_rule: ElimFn::new(|mut b| {
                let mut fields = TermsOfVariousTypes::new();
                let ctor_idx = if let Some((k, v)) = b.pop_last() {
                    let () = fields.push::<V>(v);
                    let () = fields.push::<K>(k);
                    let () = fields.push::<Self>(b);
                    2
                } else {
                    1
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
    fn visit_deep<T: Construct>(&self) -> impl Iterator<Item = T> {
        visit_self::<T, Self>(self)
            .chain(
                self.iter()
                    .flat_map(|(k, v)| k.visit_deep().chain(v.visit_deep())),
            )
            .chain({
                let mut b = self.clone();
                iter::from_fn(move || {
                    let _: (_, _) = b.pop_last()?;
                    visit_self_opt::<T, Self>(&b).cloned()
                })
            })
    }
}
