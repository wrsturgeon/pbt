//! Implementations for `Vec<_>`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        multiset::Multiset,
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{any::TypeId, num::NonZero},
};

impl<T> Pbt for Vec<T>
where
    T: Pbt,
{
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        reason = "end-users shouldn't be calling this"
    )]
    fn construct<F>(
        Parts {
            mut fields,
            variant_index,
        }: Parts<F>,
    ) -> Self
    where
        F: Fields,
    {
        let algebraic_index: usize = variant_index.expect("`Vec` is not a literal").get();
        match algebraic_index {
            1 => vec![],
            2 => {
                let mut acc: Self = fields.field();
                let () = acc.push(fields.field());
                acc
            }
            _ => panic!("can't instantiate variant #{algebraic_index} of `Vec`"),
        }
    }

    #[inline]
    fn deconstruct(mut self) -> Parts<Store> {
        let Some(caboose) = self.pop() else {
            return Parts {
                fields: Store::new(),
                variant_index: Some(const { NonZero::new(1).unwrap() }),
            };
        };
        let mut fields = Store::new();
        let () = fields.push(caboose);
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: Some(const { NonZero::new(2).unwrap() }),
        }
    }

    #[inline]
    fn register(registration: &mut Registration<'_>) -> Variants<Self> {
        let () = registration.register::<T>();
        Variants::Algebraic(vec![
            Variant {
                field_types: Multiset::new(),
            },
            Variant {
                field_types: [TypeId::of::<Self>(), TypeId::of::<T>()]
                    .into_iter()
                    .collect(),
            },
        ])
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<Vec<usize>> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<Vec<usize>> = vec![
            vec![],
            vec![],
            vec![],
            vec![],
            vec![14_075_417_872_264_614_812, 9_271_126_992_018_358_126],
            vec![5_536_629_187_452_512_295, 1_501_726_134_688_862_675],
            vec![4],
            vec![],
            vec![],
            vec![2],
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<Vec<usize>>();
    }

    #[test]
    fn eta_expansion_deep() {
        let () = check_eta_expansion::<Vec<Vec<usize>>>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<Vec<usize>>();
    }

    #[test]
    fn serialization_deep() {
        let () = check_serialization::<Vec<Vec<usize>>>();
    }
}
