//! Implementations for `Arc<_>`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    alloc::sync::Arc,
    core::{any::TypeId, iter, num::NonZero},
};

impl<T> Pbt for Arc<T>
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
        let algebraic_index: usize = variant_index.expect("`Arc` is not a literal").get();
        match algebraic_index {
            1 => Arc::new(fields.field()),
            _ => panic!("can't instantiate variant #{algebraic_index} of `Arc`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        let mut fields = Store::new();
        let () = fields.push::<T>(T::clone(&self));
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
        super::*,
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        let generated: Vec<Arc<usize>> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<Arc<usize>> = vec![
            Arc::new(0),
            Arc::new(7_804_948_724_862_110_416),
            Arc::new(17_108_568_891_541_767_080),
            Arc::new(14_756_591_828_928_955_088),
            Arc::new(1),
            Arc::new(1),
            Arc::new(10),
            Arc::new(19),
            Arc::new(13),
            Arc::new(0),
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<Arc<usize>>();
    }

    #[test]
    fn eta_expansion_deep() {
        let () = check_eta_expansion::<Arc<Arc<usize>>>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<Arc<usize>>();
    }

    #[test]
    fn serialization_deep() {
        let () = check_serialization::<Arc<Arc<usize>>>();
    }
}
