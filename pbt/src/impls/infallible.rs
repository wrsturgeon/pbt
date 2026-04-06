//! Implementation for `core::convert::Infallible`.

use {
    crate::{
        construct::{Algebraic, Construct, ElimFn, TypeFormer},
        reflection::Type,
    },
    core::{convert::Infallible, iter},
    std::collections::BTreeSet,
};

impl Construct for Infallible {
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
