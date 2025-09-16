//! Implementations for `Option<_>`.

use {
    crate::{
        ast_size::AstSize,
        edge_cases::EdgeCases,
        error,
        exhaust::Exhaust,
        impls::Either,
        max::{Max, MaybeDecidable, MaybeOverflow},
        pseudorandom::Pseudorandom,
        test_impls_for,
        value_size::ValueSize,
    },
    core::iter,
};

#[cfg(test)]
use core::convert::Infallible;

/*
impl<T: AstSize> AstSize for Option<T> {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        if matches!(*T::MAX_AST_SIZE.at_most(), Max::Uninstantiable) {
            MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)))
        } else {
            match T::MAX_AST_SIZE {
                MaybeDecidable::Decidable(_) => {
                    MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(1)))
                }
                MaybeDecidable::AtMost(_) => {
                    MaybeDecidable::AtMost(Max::Finite(MaybeOverflow::Contained(1)))
                }
            }
        };
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
        if matches!(*T::MAX_AST_SIZE.at_most(), Max::Uninstantiable) {
            MaybeDecidable::Decidable(Max::Finite(0.))
        } else {
            match T::MAX_AST_SIZE {
                MaybeDecidable::Decidable(_) => MaybeDecidable::Decidable(Max::Finite(1.)),
                MaybeDecidable::AtMost(_) => MaybeDecidable::AtMost(Max::Finite(1.)),
            }
        };
    #[inline]
    fn ast_size(&self) -> MaybeOverflow<usize> {
        MaybeOverflow::Contained(self.is_some().into())
    }
}
*/

impl<T: AstSize> AstSize for Option<T> {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        if matches!(*T::MAX_AST_SIZE.at_most(), Max::Uninstantiable) {
            MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)))
        } else {
            T::MAX_AST_SIZE
        };
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> =
        if matches!(*T::MAX_EXPECTED_AST_SIZE.at_most(), Max::Uninstantiable) {
            MaybeDecidable::Decidable(Max::Finite(0.))
        } else {
            T::MAX_EXPECTED_AST_SIZE
        };
    #[inline]
    fn ast_size(&self) -> MaybeOverflow<usize> {
        self.as_ref()
            .map_or(MaybeOverflow::Contained(0), AstSize::ast_size)
    }
}

impl<T: ValueSize> ValueSize for Option<T> {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> =
        if matches!(*T::MAX_VALUE_SIZE.at_most(), Max::Uninstantiable) {
            MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0)))
        } else {
            match T::MAX_VALUE_SIZE {
                MaybeDecidable::Decidable(decidable) => {
                    MaybeDecidable::Decidable(decidable.plus(1))
                }
                MaybeDecidable::AtMost(decidable) => MaybeDecidable::AtMost(decidable.plus(1)),
            }
        };
    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        self.as_ref().map_or(MaybeOverflow::Contained(0), |some| {
            some.value_size().plus(1)
        })
    }
}

impl<T: EdgeCases> EdgeCases for Option<T> {
    type EdgeCases =
        iter::Chain<iter::Once<Self>, iter::Map<<T as EdgeCases>::EdgeCases, fn(T) -> Self>>;
    #[inline]
    fn edge_cases() -> Self::EdgeCases {
        #[expect(
            clippy::as_conversions,
            reason = "More stringently checked for function-pointer types"
        )]
        iter::once(None).chain(<T as EdgeCases>::edge_cases().map(Some as fn(_) -> _))
    }
}

impl<T: Exhaust> Exhaust for Option<T> {
    type Exhaust = Either<iter::Map<T::Exhaust, fn(T) -> Self>, iter::Once<Self>>;
    #[inline]
    fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
        Ok(if let Some(value_size) = value_size.checked_sub(1) {
            Either::A(T::exhaust(value_size)?.map(Some))
        } else {
            Either::B(iter::once(None))
        })
    }
}

impl<T: Pseudorandom> Pseudorandom for Option<T> {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        Ok(
            if rng.next_u32().is_multiple_of(
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_possible_truncation,
                    clippy::cast_sign_loss,
                    reason = "intentional"
                )]
                (expected_ast_size as u32).saturating_add(1),
            ) {
                None
            } else {
                T::pseudorandom(expected_ast_size - 1., rng).ok()
            },
        )
    }
}

test_impls_for!(Option<Infallible>, option_infallible);
test_impls_for!(Option<()>, option_unit);
test_impls_for!(Option<u8>, option_u8);
