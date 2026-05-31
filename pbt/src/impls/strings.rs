//! Implementations for `String`.

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

impl Pbt for String {
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
        let algebraic_index: usize = variant_index.expect("`String` is not a literal").get();
        match algebraic_index {
            1 => Self::new(),
            2 => {
                let mut acc: Self = fields.field();
                let () = acc.push(fields.field());
                acc
            }
            _ => panic!("can't instantiate variant #{algebraic_index} of `String`"),
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
        let () = registration.register::<char>();
        Variants::Algebraic(vec![
            Variant {
                field_types: Multiset::new(),
            },
            Variant {
                field_types: [TypeId::of::<Self>(), TypeId::of::<char>()]
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
        let generated: Vec<String> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<String> = vec![
            String::new(),
            String::new(),
            "\u{fb8e8}\u{9bf28}\u{7ea5b}".to_owned(),
            "\u{100fee}\u{bdb4}".to_owned(),
            "\u{67457}\u{6db20}".to_owned(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
            String::new(),
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<String>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<String>();
    }
}
