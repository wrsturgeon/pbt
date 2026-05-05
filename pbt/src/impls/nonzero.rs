//! Implementations for `core::num::NonZero<_>` types.

use {
    crate::{
        pbt::{Literal, Pbt, TypeFormer, visit_self, visit_self_owned},
        reflection::{Type, register, type_of},
        scc::StronglyConnectedComponents,
        shrink::shrink,
    },
    core::num::NonZero,
    std::collections::BTreeSet,
};

/// Generate an arbitrary nonzero value for an
/// unsigned integer of fixed but unspecified width.
#[macro_export]
macro_rules! arbitrary_nonzero_unsigned {
    // TODO: iterate over a `u64` as 64 booleans
    // instead of recomputing each
    ($u:ty, $prng:ident) => {{
        let mut acc: $u = 1;

        while ($prng.rand() & 3) != 0 {
            #[allow(
                clippy::allow_attributes,
                clippy::default_numeric_fallback,
                reason = "type varies"
            )]
            if acc.cast_signed() < 0 {
                acc = <$u>::MAX;
                break;
            }
            acc <<= 1_u8;
            acc |= <$u>::from(($prng.rand() & 1) != 0);
        }
        acc
    }};
}

/// Implement `Pbt` for `NonZero<$u>`.
macro_rules! impl_for {
    ($u:ty) => {
        impl Pbt for NonZero<$u> {
            #[inline]
            fn register_all_immediate_dependencies(
                visited: &mut BTreeSet<Type>,
                sccs: &mut StronglyConnectedComponents,
            ) {
                if !visited.insert(type_of::<Self>()) {
                    return;
                }
                let () = register::<$u>(visited.clone(), sccs);
            }

            #[inline]
            fn type_former() -> TypeFormer<Self> {
                TypeFormer::Literal(Literal {
                    deserialize: |s| NonZero::new(s.parse().ok()?),
                    // SAFETY: Internals of `arbitrary_nonzero_unsigned`
                    // prevent `0` (see above).
                    generate: |prng| unsafe {
                        NonZero::new_unchecked(arbitrary_nonzero_unsigned!($u, prng))
                    },
                    serialize: |value: &Self| value.get().to_string(),
                    shrink: |u| Box::new(shrink(u.get()).filter_map(NonZero::new)),
                })
            }

            #[inline]
            fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
                visit_self(self).chain(visit_self_owned(self.get()))
            }
        }
    };
}

impl_for!(u8);
impl_for!(u16);
impl_for!(u32);
impl_for!(u64);
impl_for!(u128);
impl_for!(usize);

impl Pbt for NonZero<char> {
    #[inline]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        sccs: &mut StronglyConnectedComponents,
    ) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<char>(visited.clone(), sccs);
    }

    #[inline]
    #[expect(clippy::as_conversions, reason = "intentional")]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Literal(Literal {
            deserialize: |s| NonZero::new(s.parse().ok()?),
            generate: |prng| loop {
                let u: u32 = arbitrary_nonzero_unsigned!(u32, prng);
                if let Ok(c) = char::try_from(u) {
                    // SAFETY: Internals of `arbitrary_nonzero_unsigned`
                    // prevent `0` (see above).
                    return unsafe { NonZero::new_unchecked(c) };
                }
            },
            serialize: |value: &Self| value.get().to_string(),
            shrink: |c| {
                Box::new(
                    shrink(c.get() as u32).filter_map(|u| NonZero::new(char::try_from(u).ok()?)),
                )
            },
        })
    }

    #[inline]
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(visit_self_owned(self.get()))
    }
}
