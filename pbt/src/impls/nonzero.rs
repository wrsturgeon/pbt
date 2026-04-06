//! Implementations for `core::num::NonZero<_>` types.

use {
    crate::{
        construct::{Construct, Literal, TypeFormer, visit_self, visit_self_owned},
        reflection::{TermsOfVariousTypes, Type, register},
        shrink::shrink,
        size::Size,
    },
    core::num::NonZero,
    std::collections::BTreeSet,
    wyrand::WyRand,
};

impl Construct for NonZero<char> {
    #[inline]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        TermsOfVariousTypes::new()
    }

    #[inline]
    fn register_all_immediate_dependencies(visited: &BTreeSet<Type>) {
        let () = register::<char>(visited.clone());
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
                if let Ok(c) = char::try_from(u)
                    && let Some(nz) = NonZero::new(c)
                {
                    return nz;
                }
            },
            shrink: |c| {
                Box::new(
                    shrink(c.get() as u32).filter_map(|u| NonZero::new(char::try_from(u).ok()?)),
                )
            },
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(visit_self_owned(self.get()))
    }
}
