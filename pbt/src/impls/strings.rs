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
        crate::{
            arbitrary::arbitrary, check_eta_expansion, check_serialization, persist,
            reflection::register_globally,
        },
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let () = register_globally::<String>();
        let mut prng = WyRand::new(42);
        let mut expected: Vec<String> = persist::replay();
        let () = expected.extend([
            String::new(),
            String::new(),
            "\u{a76e5}".to_owned(),
            String::new(),
            "\u{24dea}".to_owned(),
            "\u{16efb}".to_owned(),
            "\u{e7613}\u{7ba93}".to_owned(),
            "\u{bd56d}".to_owned(),
            "\u{d0dc9}\u{499ef}".to_owned(),
            "\u{85b94}\u{bd703}\u{4be1a}".to_owned(),
        ]);
        let generated: Vec<String> = arbitrary(&mut prng).unwrap().take(expected.len()).collect();
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
