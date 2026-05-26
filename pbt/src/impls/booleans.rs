//! Implementations for `bool`.

use {
    crate::{
        multiset::Multiset,
        pbt::Pbt,
        reflection::{Erased, Variant},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::any::TypeId,
};

impl Pbt for bool {
    #[inline]
    fn variants(
        _variants: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>,
        visited: &mut HashSet<TypeId>,
    ) -> Arc<[Variant<Self>]> {
        let ty = TypeId::of::<Self>();
        if visited.insert(ty) {
            // here's where we'd run DFS iff not already in `visited`
        }
        Arc::new([
            Variant::Algebraic {
                fields: Multiset::new(),
                // TODO
            },
            Variant::Algebraic {
                fields: Multiset::new(),
                // TODO
            },
        ])
    }
}
