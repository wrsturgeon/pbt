//! Implementations for `bool`.

use {
    crate::{
        multiset::Multiset,
        pbt::Pbt,
        reflection::{Erased, Variant},
        type_id::Type,
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
};

impl Pbt for bool {
    #[inline]
    fn variants(
        _variants: &mut HashMap<Type, Arc<[Variant<Erased>]>>,
        visited: &mut HashSet<Type>,
    ) -> Arc<[Variant<Self>]> {
        let ty = Type::new::<Self>();
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
