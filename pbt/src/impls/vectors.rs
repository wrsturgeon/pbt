//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        Pbt,
        fields::Fields,
        hash::set,
        multiset::Multiset,
        reflection::{Constructor, Erased, Variant, register},
    },
    ahash::HashSet,
    alloc::{collections::BTreeMap, sync::Arc},
    core::any::TypeId,
};

impl<T> Pbt for Vec<T>
where
    T: Pbt,
{
    #[inline]
    #[expect(clippy::panic, reason = "end-users shouldn't be calling this")]
    fn instantiate_variant<F>(variant_index: usize, mut fields: F) -> Self
    where
        F: Fields,
    {
        match variant_index {
            0 => vec![],
            1 => {
                let mut acc: Self = fields.field();
                let () = acc.push(fields.field());
                acc
            }
            _ => panic!(
                "can't instantiate variant #{} of `bool`, since there are only {} variants",
                variant_index,
                Self::variants(&mut BTreeMap::new(), &mut set()).len(),
            ),
        }
    }

    #[inline]
    fn variants(
        variants: &mut BTreeMap<TypeId, Arc<[Constructor<Erased>]>>,
        visited: &mut HashSet<TypeId>,
    ) -> Vec<Variant<Self>> {
        let () = register::<T>(variants, visited);
        vec![
            Variant::Algebraic {
                field_types: Multiset::new(),
            },
            Variant::Algebraic {
                field_types: [TypeId::of::<Self>(), TypeId::of::<T>()]
                    .into_iter()
                    .collect(),
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {crate::arbitrary, pretty_assertions::assert_eq, wyrand::WyRand};

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<Vec<usize>> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<Vec<usize>> = vec![
            vec![],
            vec![],
            vec![],
            vec![10],
            vec![],
            vec![
                1_501_726_134_688_862_675,
                0,
                9_423_774_293_538_187_240,
                0,
                1,
            ],
            vec![],
            vec![],
            vec![],
            vec![],
        ];
        assert_eq!(generated, expected);
    }
}
