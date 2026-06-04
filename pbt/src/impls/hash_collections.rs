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
    std::collections::{HashMap, HashSet},
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

impl<K, V, S: 'static + BuildHasher + Clone + Default> Pbt for HashMap<K, V, S>
where
    K: Eq + Hash + Pbt,
    V: Pbt,
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
        let algebraic_index: usize = variant_index.expect("`HashMap` is not a literal").get();
        match algebraic_index {
            1 => Self::with_hasher(S::default()),
            2 => {
                let mut acc: Self = fields.field();
                let _dup: Option<V> = acc.insert(fields.field(), fields.field());
                acc
            }
            _ => panic!("can't instantiate variant #{algebraic_index} of `HashMap`"),
        }
    }

    #[inline]
    #[expect(clippy::panic, reason = "end-users shouldn't be calling this")]
    fn deconstruct(mut self) -> Parts<Store> {
        let Some(arbitrary_key) = self.keys().next().cloned() else {
            return Parts {
                fields: Store::new(),
                variant_index: Some(const { NonZero::new(1).unwrap() }),
            };
        };
        let Some(removed) = self.remove(&arbitrary_key) else {
            panic!("INTERNAL ERROR (`pbt`): TOCTOU");
        };
        let mut fields = Store::new();
        let () = fields.push(removed);
        let () = fields.push(arbitrary_key);
        let () = fields.push(self);
        Parts {
            fields,
            variant_index: Some(const { NonZero::new(2).unwrap() }),
        }
    }

    #[inline]
    fn register(registration: &mut Registration<'_>) -> Variants<Self> {
        let () = registration.register::<K>();
        let () = registration.register::<V>();
        Variants::Algebraic(vec![
            Variant {
                field_types: Multiset::new(),
            },
            Variant {
                field_types: [TypeId::of::<Self>(), TypeId::of::<K>(), TypeId::of::<V>()]
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
    fn deterministic_set() {
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
            vec![],
            vec![9_271_126_992_018_358_126, 14_075_417_872_264_614_812],
            vec![1_501_726_134_688_862_675, 5_536_629_187_452_512_295],
            vec![4],
            vec![],
            vec![],
            vec![2],
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion_set() {
        let () = check_eta_expansion::<HashSet<usize>>();
    }

    #[test]
    fn serialization_set() {
        let () = check_serialization::<HashSet<usize>>();
    }

    #[test]
    fn deterministic_map() {
        let mut prng = WyRand::new(42);
        let generated: Vec<Vec<(usize, usize)>> = arbitrary(&mut prng)
            .unwrap()
            .take(10)
            .map(|map: HashMap<usize, usize>| {
                let mut v: Vec<_> = map.into_iter().collect();
                let () = v.sort_unstable();
                v
            })
            .collect();
        let expected: Vec<Vec<(usize, usize)>> = vec![
            vec![],
            vec![],
            vec![],
            vec![],
            vec![
                (0, 4),
                (14_075_417_872_264_614_812, 9_271_126_992_018_358_126),
            ],
            vec![(1_501_726_134_688_862_675, 0)],
            vec![],
            vec![],
            vec![],
            vec![],
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion_map() {
        let () = check_eta_expansion::<HashMap<usize, usize>>();
    }

    #[test]
    fn serialization_map() {
        let () = check_serialization::<HashMap<usize, usize>>();
    }
}
