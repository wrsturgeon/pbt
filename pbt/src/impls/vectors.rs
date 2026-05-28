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
            vec![],
            vec![
                3_641_599_152_564_394_457,
                8_131_263_138_448_082_739,
                10_118_079_673_692_791_035,
            ],
            vec![
                1,
                0,
                12_263_291_316_824_364_412,
                12_102_167_028_877_106_995,
                5_323_362_331_968_575_596,
                6,
                1,
            ],
            vec![13, 1],
            vec![],
            vec![],
            vec![],
        ];
        assert_eq!(generated, expected);
    }
}
