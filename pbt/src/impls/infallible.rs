//! Implementation for `core::convert::Infallible`.

use {
    crate::{
        construct::{Algebraic, Construct, ElimFn, TypeFormer},
        reflection::{Type, type_of},
        scc::StronglyConnectedComponents,
    },
    core::{convert::Infallible, iter},
    std::collections::BTreeSet,
};

impl Construct for Infallible {
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
    fn visit_deep<V: Construct>(&self) -> impl Iterator<Item = V> {
        iter::empty()
    }
}
