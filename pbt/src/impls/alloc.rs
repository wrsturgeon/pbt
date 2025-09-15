//! Implementations for types from Rust's `alloc` crate.

/*
use {
    crate::{
        ast_size::AstSize,
        error,
        exhaust::Exhaust,
        impls::{
            slice_ast_size, slice_value_size,
            tuples::{CachingIterator, MaybeIterator, NestedIterator},
        },
        max::{Max, MaybeDecidable, MaybeOverflow},
        pseudorandom::Pseudorandom,
        test_impls_for,
        value_size::ValueSize,
    },
    alloc::vec::Vec,
    core::{iter, ops::Range},
};

#[cfg(test)]
use core::convert::Infallible;

pub enum ExhaustVec<T: Clone + Exhaust> {
    Exhausted,
    EmptyVec,
    NonEmpty {
        non_last: Vec<CachingIterator<T>>,
        last: MaybeIterator<T>,
        total_size: usize,
    },
}

impl<T: Clone + Exhaust> ExhaustVec<T> {
    #[inline]
    fn new(total_size: usize, len: usize) -> Self {
        if let Some(non_last_len) = len.checked_sub(1) {
            Self::NonEmpty {
                non_last: {
                    let mut acc = Vec::with_capacity(non_last_len);
                    for _ in 0..non_last_len {
                        let () = acc.push(CachingIterator::Inactive);
                    }
                    acc
                },
                last: MaybeIterator::Inactive,
                total_size,
            }
        } else {
            Self::EmptyVec
        }
    }
}

// TODO: Either return an iterator or use an accmulator argument
// rather than returning vectors, since the latter requires
// appending vectors N times for a vector of length N.
#[inline]
fn next_vec<T: Clone + Exhaust>(
    non_last: &mut [CachingIterator<T>],
    last: &mut MaybeIterator<T>,
    remaining_size: usize,
) -> Option<Vec<T>> {
    let [ref mut head_iter, ref mut tail_iter @ ..] = *non_last else {
        return last
            .nested_next(remaining_size)
            .map(|singleton| alloc::vec![singleton]);
    };

    // Get the cached head value, or try to create it if not cached, exiting if that fails:
    let (head_size, mut head) = head_iter.cached_or_new(remaining_size)?;

    // Subtract the head size from the remaining size (for the tail):
    // Note that this isn't using `checked_sub`, since _a priori_
    // the head can never be larger than `remaining_size`.
    // However, if this invariant were to be violated,
    // tests would pick it up, since Rust's `-` panics on overflow in debug builds.
    #[expect(clippy::arithmetic_side_effects, reason = "Intentional: see above.")]
    let remaining_size = remaining_size - head_size;

    loop {
        if let Some(mut tail) = next_vec(tail_iter, last, remaining_size) {
            let mut acc = alloc::vec![head.clone()];
            let () = acc.append(&mut tail);
            return Some(acc);
        }

        let () = head_iter.step()?;
        head = head_iter.unwrap_ref();
    }
}

impl<T: Clone + Exhaust> Iterator for ExhaustVec<T> {
    type Item = Vec<T>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            Self::Exhausted => None,
            Self::EmptyVec => {
                *self = Self::Exhausted;
                Some(alloc::vec![])
            }
            Self::NonEmpty {
                ref mut non_last,
                ref mut last,
                total_size,
            } => next_vec(non_last, last, total_size).or_else(|| {
                *self = Self::Exhausted;
                None
            }),
        }
    }
}

impl<T: AstSize> AstSize for Vec<T> {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = match T::MAX_AST_SIZE {
        MaybeDecidable::Decidable(decidable) => MaybeDecidable::Decidable(match decidable {
            Max::Uninstantiable => Max::Finite(MaybeOverflow::Contained(0)), // b/c `vec![]` always works
            Max::Finite(_) | Max::Infinite => Max::Infinite,
        }),
        MaybeDecidable::AtMost(decidable) => MaybeDecidable::AtMost(match decidable {
            Max::Uninstantiable => Max::Finite(MaybeOverflow::Contained(0)), // b/c `vec![]` always works
            Max::Finite(_) | Max::Infinite => Max::Infinite,
        }),
    };
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> = match T::MAX_EXPECTED_AST_SIZE {
        MaybeDecidable::Decidable(decidable) => MaybeDecidable::Decidable(match decidable {
            Max::Uninstantiable => Max::Finite(0.), // b/c `vec![]` always works
            Max::Finite(_) | Max::Infinite => Max::Infinite,
        }),
        MaybeDecidable::AtMost(decidable) => MaybeDecidable::AtMost(match decidable {
            Max::Uninstantiable => Max::Finite(0.), // b/c `vec![]` always works
            Max::Finite(_) | Max::Infinite => Max::Infinite,
        }),
    };

    #[inline]
    fn ast_size(&self) -> MaybeOverflow<usize> {
        slice_ast_size(self)
    }
}

impl<T: ValueSize> ValueSize for Vec<T> {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = match T::MAX_VALUE_SIZE {
        MaybeDecidable::Decidable(decidable) => MaybeDecidable::Decidable(match decidable {
            Max::Uninstantiable => Max::Finite(MaybeOverflow::Contained(0)), // b/c `vec![]` always works
            Max::Finite(_) | Max::Infinite => Max::Infinite,
        }),
        MaybeDecidable::AtMost(decidable) => MaybeDecidable::AtMost(match decidable {
            Max::Uninstantiable => Max::Finite(MaybeOverflow::Contained(0)), // b/c `vec![]` always works
            Max::Finite(_) | Max::Infinite => Max::Infinite,
        }),
    };

    #[inline]
    fn value_size(&self) -> MaybeOverflow<usize> {
        slice_value_size(self)
    }
}

impl<T: Clone + Exhaust> Exhaust for Vec<T> {
    type Exhaust = iter::FlatMap<
        iter::Zip<iter::Repeat<usize>, Range<usize>>,
        ExhaustVec<T>,
        fn((usize, usize)) -> ExhaustVec<T>,
    >;
    #[inline]
    fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
        Ok(iter::repeat(value_size)
            .zip(0..value_size)
            .flat_map((move |(total_size, len)| ExhaustVec::new(total_size, len)) as fn(_) -> _))
    }
}

impl<T: Pseudorandom> Pseudorandom for Vec<T> {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        // TODO: Big-picture, this isn't worth it, is it?

        let expected_item_ast_size = match *const { T::MAX_EXPECTED_AST_SIZE.at_most() } {
            Max::Uninstantiable => return Ok(alloc::vec![]),
            Max::Finite(finite) => finite.min(libm::sqrtf(expected_ast_size)),
            Max::Infinite => libm::sqrtf(expected_ast_size),
        };

        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_precision_loss,
            clippy::cast_sign_loss,
            clippy::modulo_arithmetic,
            reason = "intentional"
        )]
        let len = ((rng.next_u32() as f32 % (2.0 * expected_ast_size / expected_item_ast_size))
            + 0.5) as usize;

        let mut acc = Self::with_capacity(len);
        for _ in 0..len {
            let () = acc.push({
                // SAFETY:
                // Checked to be instantiable above.
                unsafe { T::pseudorandom(expected_item_ast_size, rng).unwrap_unchecked() }
            });
        }
        Ok(acc)
    }
}

test_impls_for!(Vec<Infallible>, vec_infallible);
test_impls_for!(Vec<()>, vec_unit);
test_impls_for!(Vec<u8>, vec_u8);
*/
