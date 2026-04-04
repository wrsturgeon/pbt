//! Implementation for `String<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, Literal,
            TypeFormer, visit_self, visit_self_opt, visit_self_or,
        },
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        shrink::shrink,
        size::Size,
    },
    core::{any::type_name, iter, num::NonZero},
    std::{collections::BTreeSet, ffi::CString, vec},
};

impl Construct for char {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut wyrand::WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "intentional"
    )]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            generate: |prng| loop {
                let u = prng.rand() as u32;
                if let Ok(c) = char::try_from(u) {
                    return c;
                }
            },
            shrink: |c| Box::new(shrink(c as u32).filter_map(|u| char::try_from(u).ok())),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self)
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_or(self, iter::empty)
    }
}

impl Construct for String {
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
                let () = fields.push(sizes.arbitrary::<char>(prng));
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
        let () = register::<char>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![
                IntroductionRule {
                    call: CtorFn::new(|_| String::new()),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    call: CtorFn::new(|terms| {
                        let mut acc = terms.must_pop::<Self>(); // tail
                        acc.push(terms.must_pop::<char>()); // head
                        acc
                    }),
                    immediate_dependencies: [type_of::<char>(), type_of::<Self>()]
                        .into_iter()
                        .collect(),
                },
            ],
            elimination_rule: ElimFn::new(|mut v| {
                let mut fields = TermsOfVariousTypes::new();
                let ctor_idx = if let Some(head) = v.pop() {
                    let () = fields.push::<char>(head);
                    let () = fields.push::<Self>(v);
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
            .chain(
                self.chars()
                    .flat_map(|c| visit_self(&c).collect::<Vec<_>>()),
            )
            .chain({
                let mut v = self.clone();
                iter::from_fn(move || {
                    let _: char = v.pop()?;
                    visit_self_opt::<V, Self>(&v).cloned()
                })
            })
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        #[expect(clippy::todo, reason = "TODO")]
        visit_self_or(self, || -> vec::IntoIter<_> { todo!("lifetime issues") })
    }
}

impl Construct for CString {
    #[inline]
    fn arbitrary_fields_for_ctor(
        ctor_idx: NonZero<usize>,
        prng: &mut wyrand::WyRand,
        size: Size,
    ) -> TermsOfVariousTypes {
        let mut sizes = size.partition::<Self>(ctor_idx, prng);
        let mut fields = TermsOfVariousTypes::new();
        let () = fields.push(sizes.arbitrary::<Vec<u8>>(prng));
        fields
    }

    #[inline]
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<Vec<u8>>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
                call: CtorFn::new(|terms| {
                    let mut bytes: Vec<u8> = terms.must_pop();
                    if bytes.contains(&0) {
                        let () = bytes.retain(|&b| b != 0);
                    }
                    #[expect(clippy::expect_used, reason = "logically impossible")]
                    CString::new(bytes).expect("internal `pbt` error: C-string error")
                }),
                immediate_dependencies: iter::once(type_of::<Vec<u8>>()).collect(),
            }],
            elimination_rule: ElimFn::new(|s| {
                let mut fields = TermsOfVariousTypes::new();
                let () = fields.push(s.into_bytes());
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self::<V, Self>(self)
            .chain(self.as_bytes().iter().flat_map(visit_self))
            .chain({
                let mut v = self.as_bytes().to_vec();
                iter::from_fn(move || {
                    let _: u8 = v.pop()?;
                    visit_self_opt(&v).cloned()
                })
            })
    }

    #[inline]
    fn visit_shallow<V: Construct>(&self) -> impl Iterator<Item = &V> {
        visit_self_or(self, || self.as_bytes().iter().flat_map(u8::visit_shallow))
    }
}
