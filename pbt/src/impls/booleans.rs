//! Implementations for `bool`.

use {
    crate::{
        multiset::Multiset,
        pbt::Pbt,
        reflection::{Reflection, TypeGraphVertex, Variant},
        type_id::Type,
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
};

impl Pbt for bool {
    #[inline]
    fn reflect(
        _vertices: &mut HashMap<Type, Arc<TypeGraphVertex>>,
        visited: &mut HashSet<Type>,
    ) -> Reflection<Self> {
        let ty = Type::new::<Self>();
        if visited.insert(ty) {
            // here's where we'd run DFS iff not already in `visited`
        }
        Reflection {
            variants: Box::new([
                Variant::Algebraic {
                    fields: Multiset::new(),
                    // TODO
                },
                Variant::Algebraic {
                    fields: Multiset::new(),
                    // TODO
                },
            ]),
        }
    }
}
