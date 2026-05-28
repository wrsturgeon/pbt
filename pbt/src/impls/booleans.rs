//! Implementations for `bool`.

use {
    crate::{
        fields::Fields,
        hash::{map, set},
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
    #[expect(clippy::panic, reason = "end-users shouldn't be calling this")]
    fn instantiate_variant<F>(variant_index: usize, _fields: F) -> Self
    where
        F: Fields,
    {
        match variant_index {
            0 => false,
            1 => true,
            _ => panic!(
                "can't instantiate variant #{} of `bool`, since there are only {} variants",
                variant_index,
                Self::variants(&mut map(), &mut set()).len(),
            ),
        }
    }

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
                field_types: Multiset::new(),
            },
            Variant::Algebraic {
                field_types: Multiset::new(),
            },
        ])
    }
}
