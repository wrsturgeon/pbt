//! Implementations for `bool`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        multiset::Multiset,
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::num::NonZero,
};

impl Pbt for bool {
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
        let algebraic_index: usize = variant_index.expect("`bool` is not a literal").get();
        match algebraic_index {
            1 => false,
            2 => true,
            _ => panic!("can't instantiate variant #{algebraic_index} of `bool`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        Parts {
            fields: Store::new(),
            #[expect(clippy::arithmetic_side_effects, reason = "`bool`s can be only 0 or 1")]
            // SAFETY: `bool`s can be only 0 or 1,
            // each of which can be safely incremented to a nonzero integer.
            variant_index: Some(unsafe { NonZero::new_unchecked(usize::from(self) + 1) }),
        }
    }

    #[inline]
    fn register(_registration: &mut Registration<'_>) -> Variants<Self> {
        Variants::Algebraic(vec![
            Variant {
                field_types: Multiset::new(),
            },
            Variant {
                field_types: Multiset::new(),
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
        let generated: Vec<bool> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected = vec![
            true, false, false, true, true, true, false, true, true, false,
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<bool>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<bool>();
    }
}
