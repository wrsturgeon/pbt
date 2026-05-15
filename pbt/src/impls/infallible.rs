//! Implementation for `core::convert::Infallible`.

use {
    crate::{
        pbt::{Algebraic, ElimFn, Pbt, TypeFormer},
        reflection::{Type, type_of},
        scc::StronglyConnectedComponents,
    },
    alloc::collections::BTreeSet,
    core::{convert::Infallible, iter},
};

impl Pbt for Infallible {
    #[inline]
    #[expect(
        clippy::needless_return,
        reason = "in case a function body is added later"
    )]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        _sccs: &mut StronglyConnectedComponents,
    ) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        // just in case
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![],
            elimination_rule: ElimFn::new(|infallible| match infallible { /* uninstantiable */ }),
        })
    }

    #[inline]
    fn visit_deep<V>(&self) -> impl Iterator<Item = V>
    where
        V: Pbt,
    {
        iter::empty()
    }
}
