//! Implementations for `bool`.

use {
    crate::{
        ast_size::AstSize,
        edge_cases::EdgeCases,
        error,
        exhaust::Exhaust,
        max::{Max, MaybeDecidable, MaybeOverflow},
        pseudorandom::Pseudorandom,
        shrink::Shrink,
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

impl ValueSize for bool {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(1)));

    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(usize::from(*self))
    }
}

impl EdgeCases for bool {
    type EdgeCases = <[Self; 2] as IntoIterator>::IntoIter;
    #[inline]
    fn edge_cases() -> Self::EdgeCases {
        [false, true].into_iter()
    }
}

impl Exhaust for bool {
    type Exhaust = iter::Once<Self>;
    #[inline]
    fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
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

impl Shrink for bool {
    type Shrink = <Option<Self> as IntoIterator>::IntoIter;
    #[inline]
    fn shrink(&self) -> Self::Shrink {
        self.then_some(false).into_iter()
    }
}

test_impls_for!(bool, bool);
