//! Implementations for `Box<_>`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{any::TypeId, iter, num::NonZero},
};

impl<T> Pbt for Box<T>
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
        let algebraic_index: usize = variant_index.expect("`Box` is not a literal").get();
        match algebraic_index {
            1 => Box::new(fields.field()),
            _ => panic!("can't instantiate variant #{algebraic_index} of `Box`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let () = fields.push::<T>(*self);
        Parts {
            fields,
            variant_index: Some(const { NonZero::new(1).unwrap() }),
        }
    }

    #[inline]
    fn register(registration: &mut Registration<'_>) -> Variants<Self> {
        let () = registration.register::<T>();
        Variants::Algebraic(vec![Variant {
            field_types: iter::once(TypeId::of::<T>()).collect(),
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
        let generated: Vec<Box<usize>> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<Box<usize>> = vec![
            Box::new(0),
            Box::new(7_804_948_724_862_110_416),
            Box::new(17_108_568_891_541_767_080),
            Box::new(14_756_591_828_928_955_088),
            Box::new(1),
            Box::new(1),
            Box::new(10),
            Box::new(19),
            Box::new(13),
            Box::new(0),
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<Box<usize>>();
    }

    #[test]
    fn eta_expansion_deep() {
        let () = check_eta_expansion::<Box<Box<usize>>>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<Box<usize>>();
    }

    #[test]
    fn serialization_deep() {
        let () = check_serialization::<Box<Box<usize>>>();
    }
}
