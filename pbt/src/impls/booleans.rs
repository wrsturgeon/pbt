//! Implementations for `bool`.

use {
    crate::{
        fields::Fields,
        hash::{map, set},
        multiset::Multiset,
        pbt::Pbt,
        reflection::{Constructor, Erased, Variant},
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
        _variants: &mut HashMap<TypeId, Arc<[Constructor<Erased>]>>,
        _visited: &mut HashSet<TypeId>,
    ) -> Vec<Variant<Self>> {
        vec![
            Variant::Algebraic {
                field_types: Multiset::new(),
            },
            Variant::Algebraic {
                field_types: Multiset::new(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        crate::{arbitrary, size::Size},
        core::iter,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<bool> =
            iter::repeat_with(|| arbitrary(Size::zero(), &mut prng).unwrap())
                .take(10)
                .collect();
        let expected = vec![
            true, false, false, true, false, false, false, false, true, true,
        ];
        assert_eq!(generated, expected);
    }
}
