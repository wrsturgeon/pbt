//! Implementations for `Option<_>`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        multiset::Multiset,
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{any::TypeId, iter, num::NonZero},
};

impl<T> Pbt for Option<T>
where
    T: Pbt,
{
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_in_result,
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
        let algebraic_index: usize = variant_index.expect("`Option` is not a literal").get();
        match algebraic_index {
            1 => None,
            2 => Some(fields.field()),
            _ => panic!("can't instantiate variant #{algebraic_index} of `Option`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let Some(t) = self else {
            return Parts {
                fields: Store::new(),
                variant_index: Some(const { NonZero::new(1).unwrap() }),
            };
        };
        let mut fields = Store::new();
        let () = fields.push(t);
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
                field_types: iter::once(TypeId::of::<T>()).collect(),
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
        let generated: Vec<Option<usize>> = arbitrary(&mut prng).unwrap().take(16).collect();
        let expected: Vec<Option<usize>> = vec![
            Some(17_850_812_975_400_668_360),
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            Some(1_501_726_134_688_862_675),
            Some(3),
            None,
            Some(0),
            None,
            Some(1),
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<Option<usize>>();
    }

    #[test]
    fn eta_expansion_deep() {
        let () = check_eta_expansion::<Option<Option<usize>>>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<Option<usize>>();
    }

    #[test]
    fn serialization_deep() {
        let () = check_serialization::<Option<Option<usize>>>();
    }
}
