//! Implementations for `bool`.

use {
    crate::{
        ast_size::AstSize,
        error,
        exhaust::Exhaust,
        max::{Max, MaybeDecidable, MaybeOverflow},
        pseudorandom::Pseudorandom,
        test_impls_for,
        value_size::ValueSize,
    },
    core::iter,
};

impl AstSize for bool {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)));
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
        MaybeDecidable::Decidable(Max::Finite(0.));

    #[inline]
    fn ast_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(0)
    }
}
impl Exhaust for bool {
    #[inline]
    fn exhaust(value_size: usize) -> Result<impl Iterator<Item = Self>, error::UnreachableSize> {
        match value_size {
            0 => Ok(iter::once(false)),
            1 => Ok(iter::once(true)),
            _ => Err(error::UnreachableSize),
        }
    }
}
impl Pseudorandom for bool {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        _expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        Ok(rng.next_u32() & 1 != 0)
    }
}
impl ValueSize for bool {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(1)));

    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(usize::from(*self))
    }
}
test_impls_for!(bool, bool);
