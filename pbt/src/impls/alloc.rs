//! Implementations for types from Rust's `alloc` crate.

extern crate alloc;

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
    alloc::{boxed::Box, vec::Vec},
    core::hint::unreachable_unchecked,
};

#[cfg(test)]
use core::convert::Infallible;

/// Non-empty list of iterators,
/// with a `MaybeIterator<T>` at the end
/// and all `CachingIterator<T>`s before it.
pub enum NonEmptyIterList<T: Clone + Exhaust> {
    /// At least one `CachingIterator<T>`, then eventually a `MaybeIterator<T>`.
    Cons {
        /// The first iterator (a `CachingIterator<T>`).
        head: CachingIterator<T>,
        /// All iterators after the first, ending with a `MaybeIterator<T>`.
        tail: Box<Self>,
    },
    /// Exactly one iterator (`MaybeIterator<T>`).
    Singleton(MaybeIterator<T>),
}

/// Non-empty list of values, all but the last of which are cached.
pub enum NonEmptyCacheList<T> {
    /// More than one value.
    Cons {
        /// The first value.
        head: T,
        /// All values after the first.
        tail: Box<Self>,
    },
    /// Exactly one value.
    Singleton(T),
}

/// Exhaustively iterate over all vectors of a given value-size.
pub struct ExhaustVec<T: Clone + Exhaust> {
    /// An iterator producing linked lists.
    linked_list_iterator: Option<NonEmptyIterList<T>>,
    /// The total value-size of each vector as a whole.
    total_size: usize,
}

impl<T> From<NonEmptyCacheList<T>> for Vec<T> {
    #[inline]
    fn from(mut value: NonEmptyCacheList<T>) -> Self {
        let mut acc: Self = alloc::vec![];
        loop {
            match value {
                NonEmptyCacheList::Singleton(last) => {
                    let () = acc.push(last);
                    return acc;
                }
                NonEmptyCacheList::Cons { head, tail } => {
                    let () = acc.push(head);
                    value = *tail;
                }
            }
        }
    }
}

impl<T: Clone + Exhaust> NestedIterator for NonEmptyIterList<T> {
    type Item = NonEmptyCacheList<T>;

    #[inline]
    #[cfg(debug_assertions)]
    #[expect(clippy::panic, reason = "intentional: this is an assertion")]
    fn debug_assert_all_inactive(&self) {
        match *self {
            Self::Singleton(ref maybe_iterator) => maybe_iterator.debug_assert_all_inactive(),
            Self::Cons { ref head, ref tail } => {
                match *head {
                    CachingIterator::Active {
                        size, ref cache, ..
                    } => panic!(
                        "Expected all downstream iterators to be inactive, but a caching iterator was active with size {size:?} and cache {cache:?}",
                    ),
                    CachingIterator::Inactive => tail.debug_assert_all_inactive(),
                }
                let () = tail.debug_assert_all_inactive();
            }
        }
    }

    #[inline]
    fn nested_next(&mut self, remaining_size: usize) -> Option<Self::Item> {
        // Try to produce a singleton as long as possible,
        // then extend to a multi-element list only after
        // all singletons have been exhausted.
        if let Self::Singleton(ref mut last) = *self {
            if let Some(last) = last.next_or_new(remaining_size) {
                return Some(NonEmptyCacheList::Singleton(last));
            }
            let head_size = remaining_size.checked_sub(1)?;
            let mut head = CachingIterator::Inactive;
            let () = head.fill_cache_with_next_value(Some(head_size))?;
            let tail = Box::new(Self::Singleton(MaybeIterator::Inactive));
            *self = Self::Cons { head, tail };
        }

        // Subtract one to compensate for the value-size penalty for extending the vector.
        // Note that this isn't using `checked_sub`, since
        // the above would have exited if `remaining_size` were 0.
        // However, if this invariant were to be violated,
        // tests would pick it up, since Rust's `-` panics on overflow in debug builds.
        #[expect(clippy::arithmetic_side_effects, reason = "Intentional: see above.")]
        let remaining_size = remaining_size - 1;

        // LOOP INVARIANT (established above):
        // `head_iter` is `Cons` with an `Active` head with a value in its `cache` (as `Some(..)`).
        // If this fails at any point, this function returns `None`.
        loop {
            // Get the cached head value, which we know exists b/c of the loop invariant above:
            let Self::Cons {
                head: ref mut head_iter,
                tail: ref mut tail_iter,
            } = *self
            else {
                // SAFETY:
                // Just established above (before the loop),
                // and the end of the loop re-establishes the invariant as well.
                unsafe { unreachable_unchecked() }
            };

            let CachingIterator::Active {
                size: head_size,
                cache: Some(ref head_cached),
                ..
            } = *head_iter
            else {
                // SAFETY:
                // Just established above (before the loop),
                // and the end of the loop re-establishes the invariant as well.
                unsafe { unreachable_unchecked() }
            };

            // Subtract the head size from the remaining size (for the tail):
            // Note that this isn't using `checked_sub`, since _a priori_
            // the head can never be larger than `remaining_size`.
            // However, if this invariant were to be violated,
            // tests would pick it up, since Rust's `-` panics on overflow in debug builds.
            #[expect(clippy::arithmetic_side_effects, reason = "Intentional: see above.")]
            let tail_size = remaining_size - head_size;

            if let Some(tail) = tail_iter.nested_next(tail_size) {
                return Some(NonEmptyCacheList::Cons {
                    head: head_cached.clone(),
                    tail: Box::new(tail),
                });
            }

            // Check that all downstream iterators are inactive and ready to be restarted:
            #[cfg(debug_assertions)]
            let () = tail_iter.debug_assert_all_inactive();

            // Then update the cache with the next value for this index of the tuple,
            // implicitly restarting all downstream iterators (checked above):
            let Some(()) = head_iter.fill_cache_with_next_value(head_size.checked_sub(1)) else {
                *self = Self::Singleton(MaybeIterator::Inactive);
                return None;
            };
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take years")]
impl<T: Clone + Exhaust> Iterator for ExhaustVec<T> {
    type Item = Vec<T>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(remaining_size) = self.total_size.checked_sub(1) {
            self.linked_list_iterator
                .as_mut()
                .and_then(|iter| iter.nested_next(remaining_size))
                .map(Into::into)
                .or_else(|| {
                    self.linked_list_iterator = None;
                    None
                })
        } else {
            self.linked_list_iterator.take().map(|_| alloc::vec![])
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
    type Exhaust = ExhaustVec<T>;
    #[inline]
    fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
        if MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(value_size)))
            > Self::MAX_VALUE_SIZE
        {
            Err(error::UnreachableSize)
        } else {
            Ok(ExhaustVec {
                linked_list_iterator: Some(NonEmptyIterList::Singleton(MaybeIterator::Inactive)),
                total_size: value_size,
            })
        }
    }
}

impl<T: Pseudorandom> Pseudorandom for Vec<T> {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        // It's useful to split the total expected AST size into
        // the AST size of individual elements and the length of the vector itself.
        // If we have a random variable `L` (length) and an expected element size `E`,
        // then the total AST size is `L + (L * E)`.
        // Let's let `T` be the total. Then we want to solve `T = L + (L * E)`.
        // Pulling out an `L`, that's `T = L(1 + E)`.
        // Dividing, that's `L = T / (1 + E)`.
        // We don't want either factor (length or element size) to overtake the other too quickly
        // (e.g. very long vectors of tiny elements or singletons of massive elements),
        // and `L` and `E` are independent, so let's set `E` to at most the square root of the total.
        // TODO: Big-picture, is this `sqrt` call worth it?

        let expected_item_ast_size = match *const { T::MAX_EXPECTED_AST_SIZE.at_most() } {
            Max::Uninstantiable => return Ok(alloc::vec![]),
            Max::Finite(finite) => finite.min(libm::sqrtf(expected_ast_size)),
            Max::Infinite => libm::sqrtf(expected_ast_size),
        };

        #[expect(
            clippy::arithmetic_side_effects,
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::integer_division_remainder_used,
            reason = "intentional"
        )]
        let len = {
            let expected_ast_size_from_len = expected_ast_size / (1. + expected_item_ast_size);
            let modulo_cap = (2. * expected_ast_size_from_len + 1.5) as usize;
            (rng.next_u32() as usize) % modulo_cap
        };

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

#[cfg(test)]
#[expect(
    clippy::unwrap_used,
    reason = "Tests are supposed to fail if they don't behave as expected."
)]
mod test {
    use super::*;

    #[test]
    fn vec_infallible_size_0() {
        let mut iter = Vec::<Infallible>::exhaust(0).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_infallible_size_1() {
        assert!(
            Vec::<Infallible>::exhaust(1).is_err(),
            "No `Vec<Infallible>` of size 1 should be possible, but `Vec::<Infallible>::exhaust(1)` returned `Ok(..)`",
        );
    }

    #[test]
    fn vec_unit_size_0() {
        let mut iter = Vec::<()>::exhaust(0).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_unit_size_1() {
        let mut iter = Vec::<()>::exhaust(1).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![()]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_unit_size_2() {
        let mut iter = Vec::<()>::exhaust(2).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![(), ()]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_unit_size_3() {
        let mut iter = Vec::<()>::exhaust(3).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![(), (), ()]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_unit_size_4() {
        let mut iter = Vec::<()>::exhaust(4).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![(), (), (), ()]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_u8_size_0() {
        let mut iter = Vec::<u8>::exhaust(0).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_u8_size_1() {
        let mut iter = Vec::<u8>::exhaust(1).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![0]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_u8_size_2() {
        let mut iter = Vec::<u8>::exhaust(2).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![1]));
        assert_eq!(iter.next(), Some(alloc::vec![0, 0]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_u8_size_3() {
        let mut iter = Vec::<u8>::exhaust(3).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![2]));
        assert_eq!(iter.next(), Some(alloc::vec![1, 0]));
        assert_eq!(iter.next(), Some(alloc::vec![0, 1]));
        assert_eq!(iter.next(), Some(alloc::vec![0, 0, 0]));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn vec_u8_size_4() {
        let mut iter = Vec::<u8>::exhaust(4).unwrap();
        assert_eq!(iter.next(), Some(alloc::vec![3]));
        assert_eq!(iter.next(), Some(alloc::vec![2, 0]));
        assert_eq!(iter.next(), Some(alloc::vec![1, 1]));
        assert_eq!(iter.next(), Some(alloc::vec![1, 0, 0]));
        assert_eq!(iter.next(), Some(alloc::vec![0, 2]));
        assert_eq!(iter.next(), Some(alloc::vec![0, 1, 0]));
        assert_eq!(iter.next(), Some(alloc::vec![0, 0, 1]));
        assert_eq!(iter.next(), Some(alloc::vec![0, 0, 0, 0]));
        assert_eq!(iter.next(), None);
    }
}
