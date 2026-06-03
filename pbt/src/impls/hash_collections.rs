//! Implementations for `Hash*<..>`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        multiset::Multiset,
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{
        any::TypeId,
        hash::{BuildHasher, Hash},
        num::NonZero,
    },
    std::collections::HashSet,
};

impl<T, S: 'static + BuildHasher + Clone + Default> Pbt for HashSet<T, S>
where
    T: Eq + Hash + Pbt,
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
        let algebraic_index: usize = variant_index.expect("`HashSet` is not a literal").get();
        match algebraic_index {
            1 => Self::with_hasher(S::default()),
            2 => {
                let mut acc: Self = fields.field();
                let _dup: bool = acc.insert(fields.field());
                acc
            }
            _ => panic!("can't instantiate variant #{algebraic_index} of `HashSet`"),
        }
    }

    #[inline]
    fn deconstruct(mut self) -> Parts<Store> {
        let Some(arbitrary_key) = self.iter().next().cloned() else {
            return Parts {
                fields: Store::new(),
                variant_index: Some(const { NonZero::new(1).unwrap() }),
            };
        };
        let _present: bool = self.remove(&arbitrary_key);
        let mut fields = Store::new();
        let () = fields.push(arbitrary_key);
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
        super::*,
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<Vec<usize>> = arbitrary(&mut prng)
            .unwrap()
            .take(10)
            .map(|set: HashSet<usize>| {
                let mut v: Vec<_> = set.into_iter().collect();
                let () = v.sort_unstable();
                v
            })
            .collect();
        let expected: Vec<Vec<usize>> = vec![
            vec![],
            vec![],
            vec![],
            vec![0, 9_271_126_992_018_358_126, 14_075_417_872_264_614_812],
            vec![0, 1_501_726_134_688_862_675],
            vec![0, 1, 4_611_926_216_761_736_595, 16_688_242_715_256_209_604],
            vec![],
            vec![],
            vec![],
            vec![],
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<HashSet<usize>>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<HashSet<usize>>();
    }
}
