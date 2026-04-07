//! Implementations for `core::num::NonZero<_>` types.

use {
    crate::{
        construct::{Construct, Literal, TypeFormer, visit_self, visit_self_owned},
        reflection::{Type, register, type_of},
        shrink::shrink,
    },
    core::num::NonZero,
    std::collections::BTreeSet,
};

impl Construct for NonZero<u8> {
    #[inline]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<u8>(visited.clone());
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
                let u = prng.rand() as u8;
                if let Some(nz) = NonZero::new(u) {
                    return nz;
                }
            },
            shrink: |u| Box::new(shrink(u.get()).filter_map(NonZero::new)),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(visit_self_owned(self.get()))
    }
}

impl Construct for NonZero<char> {
    #[inline]
    fn register_all_immediate_dependencies(visited: &mut BTreeSet<Type>) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
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
