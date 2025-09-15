//! Implementations for purposefully uninstantiable types.

use {
    crate::{
        ast_size::AstSize,
        error,
        exhaust::Exhaust,
        max::{Max, MaybeDecidable, MaybeOverflow},
        pseudorandom::Pseudorandom,
        value_size::ValueSize,
    },
    core::{convert::Infallible, hint::unreachable_unchecked, iter},
};

impl AstSize for Infallible {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Uninstantiable);
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
        MaybeDecidable::Decidable(Max::Uninstantiable);
    #[inline]
    fn ast_size(&self) -> MaybeOverflow<usize> {
        // SAFETY:
        // Uninhabited type.
        unsafe { unreachable_unchecked() }
    }
}

impl ValueSize for Infallible {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Uninstantiable);
    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        // SAFETY:
        // Uninhabited type.
        unsafe { unreachable_unchecked() }
    }
}

impl Exhaust for Infallible {
    type Exhaust = iter::Empty<Self>;
    #[inline]
    fn exhaust(_value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
        Err(error::UnreachableSize)
    }
}

impl Pseudorandom for Infallible {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        _expected_ast_size: f32,
        _rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        Err(error::Uninstantiable)
    }
}
