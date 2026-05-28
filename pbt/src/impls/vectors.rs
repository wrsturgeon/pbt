//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        Pbt,
        fields::Fields,
        hash::{map, set},
        multiset::Multiset,
        reflection::{Constructor, Erased, Variant, register},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
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
                Self::variants(&mut map(), &mut set()).len(),
            ),
        }
    }

    #[inline]
    fn variants(
        variants: &mut HashMap<TypeId, Arc<[Constructor<Erased>]>>,
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

    use {
        crate::{arbitrary, size::Size},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<Vec<usize>> = Size::increasing()
            .skip(10)
            .take(10)
            .map(|size| arbitrary(size, &mut prng).unwrap())
            .collect();
        let expected: Vec<Vec<usize>> = vec![
            vec![0, 0],
            vec![],
            vec![
                15_312_978_602_927_583_178,
                0,
                0,
                4_942_278_377_568_497_097,
                12_203_955_863_004_621_295,
                2,
                3,
                0,
            ],
            vec![],
            vec![],
            vec![
                13_318_571_059_701_913_151,
                16_579_479_697_546_634_183,
                587_989_459_796_176_642,
                18_020_319_373_362_271_627,
                0,
                5_718_636_053_636_895_365,
                0,
                5_199_299_979_368_977_360,
            ],
            vec![],
            vec![0],
            vec![0, 0, 1, 0],
            vec![0, 15_514_403_627_301_449_867, 0],
        ];
        assert_eq!(generated, expected);
    }
}
