//! Implementation for `Hash{Map, Set}`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, TypeFormer,
            visit_self, visit_self_opt, visit_self_or,
        },
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        size::Size,
    },
    core::{
        any::type_name,
        hash::{BuildHasher, Hash},
        iter,
        num::NonZero,
    },
    std::collections::{BTreeSet, HashMap, HashSet},
};

impl<T: Construct + Hash, S: 'static + BuildHasher + Clone + Default> Construct for HashSet<T, S> {
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
                    call: CtorFn::new(|_| HashSet::with_hasher(S::default())),
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
                let ctor_idx = if let Some(t) = b.iter().next() {
                    let t = t.clone();
                    let _: bool = b.remove(&t);
                    let () = fields.push::<T>(t);
                    let () = fields.push::<Self>(b);
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
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self::<V, Self>(self)
            .chain(self.iter().flat_map(T::visit_deep))
            .chain({
                let mut b = self.clone();
                iter::from_fn(move || {
                    let t: T = b.iter().next()?.clone();
                    let _: bool = b.remove(&t);
                    visit_self_opt::<V, Self>(&b).cloned()
                })
            })
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_or(self, || self.iter().flat_map(T::visit_shallow))
    }
}

impl<K: Construct + Hash, V: Construct, S: 'static + BuildHasher + Clone + Default> Construct
    for HashMap<K, V, S>
{
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
                    call: CtorFn::new(|_| HashMap::with_hasher(S::default())),
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
                let ctor_idx = if let Some(k) = b.keys().next() {
                    let k = k.clone();
                    #[expect(
                        clippy::expect_used,
                        reason = "logically impossible with a mutable reference"
                    )]
                    let v = b
                        .remove(&k)
                        .expect("internal `pbt` error: schrodinger's hash map");
                    let () = fields.push::<V>(v);
                    let () = fields.push::<K>(k);
                    let () = fields.push::<Self>(b);
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
    fn visit_deep<T: Construct>(&self) -> impl Iterator<Item = T> {
        visit_self::<T, Self>(self)
            .chain(
                self.iter()
                    .flat_map(|(k, v)| k.visit_deep().chain(v.visit_deep())),
            )
            .chain({
                let mut b = self.clone();
                iter::from_fn(move || {
                    let k: K = b.keys().next()?.clone();
                    let _: Option<V> = b.remove(&k);
                    visit_self_opt::<T, Self>(&b).cloned()
                })
            })
    }

    #[inline]
    fn visit_shallow<T: Construct>(&self) -> impl Iterator<Item = &T> {
        visit_self_or(self, || {
            self.iter()
                .flat_map(|(k, v)| k.visit_shallow().chain(v.visit_shallow()))
        })
    }
}
