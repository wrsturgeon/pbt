//! Implementations for `[_; _]`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{any::TypeId, array, num::NonZero},
};

impl<T, const N: usize> Pbt for [T; N]
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
        let algebraic_index: usize = variant_index.expect("`[_; _]` is not a literal").get();
        match algebraic_index {
            1 => array::from_fn(|_i| fields.field()),
            _ => panic!("can't instantiate variant #{algebraic_index} of `[_; _]`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        for t in self.into_iter().rev() {
            let () = fields.push(t);
        }
        Parts {
            fields,
            variant_index: Some(const { NonZero::new(1).unwrap() }),
        }
    }

    #[inline]
    fn register(registration: &mut Registration<'_>) -> Variants<Self> {
        let () = registration.register::<T>();
        Variants::Algebraic(vec![Variant {
            field_types: [TypeId::of::<T>(); N].into_iter().collect(),
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
        let generated: Vec<[bool; 3]> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<[bool; 3]> = vec![
            [true, true, true],
            [true, true, true],
            [true, true, true],
            [false, false, false],
            [false, false, false],
            [false, false, false],
            [true, true, true],
            [true, true, true],
            [true, true, true],
            [true, true, true],
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<[usize; 3]>();
    }

    #[test]
    fn eta_expansion_deep() {
        let () = check_eta_expansion::<Vec<[usize; 3]>>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<[usize; 3]>();
    }

    #[test]
    fn serialization_deep() {
        let () = check_serialization::<Vec<[usize; 3]>>();
    }
}
