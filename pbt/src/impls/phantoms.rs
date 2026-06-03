//! Implementations for `PhantomData<_>`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        multiset::Multiset,
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{marker::PhantomData, num::NonZero},
};

impl<T> Pbt for PhantomData<T>
where
    T: 'static,
{
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::panic,
        reason = "end-users shouldn't be calling this"
    )]
    fn construct<F>(Parts { variant_index, .. }: Parts<F>) -> Self
    where
        F: Fields,
    {
        let algebraic_index: usize = variant_index.expect("`PhantomData` is not a literal").get();
        match algebraic_index {
            1 => Self,
            _ => panic!("can't instantiate variant #{algebraic_index} of `PhantomData`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        Parts {
            fields: Store::new(),
            variant_index: Some(const { NonZero::new(1).unwrap() }),
        }
    }

    #[inline]
    fn register(_registration: &mut Registration<'_>) -> Variants<Self> {
        // let () = registration.register::<T>(); // `T` doesn't necessarily implement `Pbt`
        Variants::Algebraic(vec![Variant {
            field_types: Multiset::new(),
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
        let generated: Vec<PhantomData<usize>> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<PhantomData<usize>> = vec![PhantomData; 10];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<PhantomData<usize>>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<PhantomData<usize>>();
    }
}
