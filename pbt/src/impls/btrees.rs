//! Implementation for `BTree{Map, Set}`.

use {
    crate::{
        multiset::Multiset,
        pbt::{
            Algebraic, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt, TypeFormer,
            push_arbitrary_field, visit_self, visit_self_opt,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    core::{iter, num::NonZero},
    std::collections::{BTreeMap, BTreeSet},
};

impl<T: Pbt + Ord> Pbt for BTreeSet<T> {
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
                    call: CtorFn::new(|_| Some(BTreeSet::new())),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    arbitrary_fields: |prng, mut sizes| {
                        let mut fields = TermsOfVariousTypes::new();
                        push_arbitrary_field::<T>(&mut fields, &mut sizes, prng)?;
                        push_arbitrary_field::<Self>(&mut fields, &mut sizes, prng)?;
                        Ok(fields)
                    },
                    call: CtorFn::new(|terms| {
                        let mut acc = terms.must_pop::<Self>(); // tail
                        acc.insert(terms.must_pop::<T>()); // head
                        Some(acc)
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
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
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

impl<K: Pbt + Ord, V: Pbt> Pbt for BTreeMap<K, V> {
    #[inline]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        sccs: &mut StronglyConnectedComponents,
    ) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<K>(visited.clone(), sccs);
        let () = register::<V>(visited.clone(), sccs);
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![
                IntroductionRule {
                    arbitrary_fields: |_, _| Ok(TermsOfVariousTypes::new()),
                    call: CtorFn::new(|_| Some(BTreeMap::new())),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    arbitrary_fields: |prng, mut sizes| {
                        let mut fields = TermsOfVariousTypes::new();
                        push_arbitrary_field::<K>(&mut fields, &mut sizes, prng)?;
                        push_arbitrary_field::<V>(&mut fields, &mut sizes, prng)?;
                        push_arbitrary_field::<Self>(&mut fields, &mut sizes, prng)?;
                        Ok(fields)
                    },
                    call: CtorFn::new(|terms| {
                        let mut acc = terms.must_pop::<Self>();
                        acc.insert(terms.must_pop::<K>(), terms.must_pop::<V>());
                        Some(acc)
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
    fn visit_deep<T: Pbt>(&self) -> impl Iterator<Item = T> {
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
