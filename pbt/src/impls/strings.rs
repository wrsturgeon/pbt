//! Implementation for `String<_>`.

use {
    crate::{
        construct::{
            Algebraic, Construct, CtorFn, Decomposition, ElimFn, IntroductionRule, Literal,
            TypeFormer, visit_self, visit_self_opt,
        },
        multiset::Multiset,
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        shrink::shrink,
    },
    core::{iter, mem, num::NonZero},
    std::{collections::BTreeSet, ffi::CString, vec},
};

impl Construct for char {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

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
}

impl Construct for String {
    #[inline]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<char>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![
                IntroductionRule {
                    arbitrary_fields: |_, _| TermsOfVariousTypes::new(),
                    call: CtorFn::new(|_| Some(String::new())),
                    immediate_dependencies: Multiset::new(),
                },
                IntroductionRule {
                    arbitrary_fields: |prng, mut sizes| {
                        let mut fields = TermsOfVariousTypes::new();
                        fields.push(sizes.arbitrary::<char>(prng));
                        fields.push(sizes.arbitrary::<Self>(prng));
                        fields
                    },
                    call: CtorFn::new(|terms| {
                        let mut acc = terms.must_pop::<Self>(); // tail
                        acc.push(terms.must_pop::<char>()); // head
                        Some(acc)
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
}

impl Construct for CString {
    #[inline]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<Vec<NonZero<u8>>>(visited.clone());
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
                arbitrary_fields: |prng, mut sizes| {
                    let mut fields = TermsOfVariousTypes::new();
                    fields.push(sizes.arbitrary::<Vec<NonZero<u8>>>(prng));
                    fields
                },
                call: CtorFn::new(|terms| {
                    let bytes: Vec<NonZero<u8>> = terms.must_pop();
                    // SAFETY: `NonZero<_>` is `repr(transparent)`.
                    let bytes: Vec<u8> =
                        unsafe { mem::transmute::<Vec<NonZero<u8>>, Vec<u8>>(bytes) };
                    #[expect(clippy::expect_used, reason = "logically impossible")]
                    Some(CString::new(bytes).expect("internal `pbt` error: C-string error"))
                }),
                immediate_dependencies: iter::once(type_of::<Vec<NonZero<u8>>>()).collect(),
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
}
