//! Implementations for `core::convert::Infallible`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variants},
        registration::Registration,
    },
    core::convert::Infallible,
};

impl Pbt for Infallible {
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
        let _algebraic_index: usize = variant_index
            .expect("`core::convert::Infallible` is not a literal")
            .get();
        panic!("can't instantiate `core::convert::Infallible`")
    }

    #[inline]
    fn deconstruct(self) -> Parts<Store> {
        match self {}
    }

    #[inline]
    fn register(_registration: &mut Registration<'_>) -> Variants<Self> {
        Variants::Algebraic(vec![])
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        wyrand::WyRand,
    };

    #[test]
    fn deterministic() {
        let mut prng = WyRand::new(42);
        assert!(arbitrary::<Infallible>(&mut prng).is_err());
    }

    #[test]
    fn eta_expansion() {
        let () = check_eta_expansion::<Infallible>();
    }

    #[test]
    fn serialization() {
        let () = check_serialization::<Infallible>();
    }
}
