//! Implementations for built-in fixed-width integer types like `u8`, `isize`, etc.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        multiset::Multiset,
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::any::TypeId,
};

impl<T> Pbt for Vec<T>
where
    T: Pbt,
{
    #[inline]
    #[expect(clippy::panic, reason = "end-users shouldn't be calling this")]
    fn construct<F>(
        Parts {
            mut fields,
            variant_index,
        }: Parts<F>,
    ) -> Self
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
            _ => panic!("can't instantiate variant #{variant_index} of `bool`"),
        }
    }

    #[inline]
    fn deconstruct(mut self) -> Parts<Store> {
        let Some(caboose) = self.pop() else {
            return Parts {
                fields: Store::new(),
                variant_index: 0,
            };
        };
        let mut fields = Store::new();
        let () = fields.push(caboose);
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: 1,
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
        crate::{arbitrary::arbitrary, check_eta_expansion},
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

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<Vec<usize>>();
    }

    #[test]
    fn eta_expansion_deep() {
        let () = check_eta_expansion::<Vec<Vec<usize>>>();
    }
}
