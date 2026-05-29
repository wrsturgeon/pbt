//! Implementations for `bool`.

use crate::{
    Pbt,
    fields::{Fields, Store},
    multiset::Multiset,
    reflection::{Parts, Reflection, Variant},
    registration::Registration,
};

impl Pbt for bool {
    #[inline]
    #[expect(clippy::panic, reason = "end-users shouldn't be calling this")]
    fn construct<F>(Parts { variant_index, .. }: Parts<F>) -> Self
    where
        F: Fields,
    {
        match variant_index {
            0 => false,
            1 => true,
            _ => panic!("can't instantiate variant #{variant_index} of `bool`"),
        }
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        Parts {
            fields: Store::new(),
            variant_index: usize::from(self),
        }
    }

    #[inline]
    fn register(_registration: &mut Registration<'_>) -> Reflection<Self> {
        Reflection {
            variants: vec![
                Variant::Algebraic {
                    field_types: Multiset::new(),
                },
                Variant::Algebraic {
                    field_types: Multiset::new(),
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        crate::{arbitrary, check_eta_expansion},
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
}
