//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        fields::Fields,
        hash::{map, set},
        multiset::Multiset,
        pbt::Pbt,
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
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<Vec<usize>> = Size::increasing()
            .skip(10)
            .take(5)
            .map(|size| arbitrary(size, &mut prng).unwrap())
            .collect();
        let expected = vec![
            // Swarm chose only "small" ctor:
            vec![6, 1, 0, 3, 0, 0, 0, 1, 4, 0, 4, 0, 0],
            vec![8_874_680_670_791_162_490],
            // Swarm chose only "large" ctor:
            vec![
                6_287_850_226_796_494_546,
                2_724_777_745_923_630_925,
                13_184_130_373_294_052_496,
                9_916_920_105_481_182_494,
                15_385_258_861_293_632_596,
                7_559_790_060_935_305_184,
                4_221_850_974_886_774_210,
            ],
            vec![],
            // Swarm allowed both ctors:
            vec![
                7_702_579_084_828_342_914,
                10_039_355_457_277_975_471,
                17_462_761_597_925_519_153,
                30,
                17_602_881_038_495_905_289,
                0,
                6_393_624_268_709_872_650,
                0,
                0,
                4,
                9_896_597_600_884_253_224,
                1,
                1,
                782_050_734_448_904_823,
                13_301_953_035_742_142_175,
                1,
            ],
        ];
        assert_eq!(generated, expected);
    }
}
