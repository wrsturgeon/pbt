//! Implementations for `(_, _)`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{any::TypeId, num::NonZero},
};

impl<A, B> Pbt for (A, B)
where
    A: Pbt,
    B: Pbt,
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
        let algebraic_index: usize = variant_index.expect("`(_, _)` is not a literal").get();
        match algebraic_index {
            1 => (fields.field(), fields.field()),
            _ => panic!("can't instantiate variant #{algebraic_index} of `(_, _)`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let (a, b) = self;
        let () = fields.push::<A>(a);
        let () = fields.push::<B>(b);
        Parts {
            fields,
            variant_index: Some(const { NonZero::new(1).unwrap() }),
        }
    }

    #[inline]
    fn register(registration: &mut Registration<'_>) -> Variants<Self> {
        let () = registration.register::<A>();
        let () = registration.register::<B>();
        Variants::Algebraic(vec![Variant {
            field_types: [TypeId::of::<A>(), TypeId::of::<B>()].into_iter().collect(),
        }])
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
        let generated: Vec<(usize, bool)> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<(usize, bool)> = vec![
            (1, true),
            (17_728_079_043_341_149_863, false),
            (3_455_211_640_292_790_292, true),
            (0, false),
            (0, false),
            (3, false),
            (6_984_722_224_437_650_403, false),
            (0, false),
            (0, false),
            (1, false),
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<(usize, bool)>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<(usize, bool)>();
    }
}
