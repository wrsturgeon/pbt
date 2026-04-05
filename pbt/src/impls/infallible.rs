//! Implementation for `core::convert::Infallible`.

use {
    crate::{
        construct::{Algebraic, Construct, ElimFn, TypeFormer},
        reflection::{TermsOfVariousTypes, Type},
        size::Size,
    },
    core::{any::type_name, convert::Infallible, iter, num::NonZero},
    std::collections::BTreeSet,
};

impl Construct for Infallible {
    #[inline]
    #[expect(clippy::panic, reason = "internal invariant violated")]
    fn arbitrary_fields_for_ctor(
        _ctor_idx: NonZero<usize>,
        _prng: &mut wyrand::WyRand,
        _size: Size,
    ) -> TermsOfVariousTypes {
        panic!(
            "internal `pbt` error: constructing an uninstantiable type (`{}`)",
            type_name::<Self>(),
        )
    }

    #[inline]
    fn register_all_immediate_dependencies(_visited: &BTreeSet<Type>) {}

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![],
            elimination_rule: ElimFn::new(|infallible| match infallible { /* uninstantiable */ }),
        })
    }

    #[inline]
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        iter::empty()
    }
}
