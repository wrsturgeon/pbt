//! Implementations for common types.

mod bool;
mod floats;
pub mod ints;
mod option;
mod tuples;
mod void;

#[cfg(feature = "alloc")]
mod alloc;

#[cfg(feature = "alloc")]
use crate::{ast_size::AstSize, max::MaybeOverflow, value_size::ValueSize};

/// One of two types, as an `enum` that tags which one is active.
/// Usually used to return one of two different iterator structures.
#[expect(
    clippy::min_ident_chars,
    reason = "A and B make as much sense as anything else."
)]
pub enum Either<A, B> {
    /// The first of two types.
    A(A),
    /// The second of two types.
    B(B),
}

#[expect(clippy::missing_trait_methods, reason = "would take years")]
impl<A: Iterator, B: Iterator<Item = A::Item>> Iterator for Either<A, B> {
    type Item = A::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            Self::A(ref mut a_iter) => a_iter.next(),
            Self::B(ref mut b_iter) => b_iter.next(),
        }
    }
}

/// AST size of a slice.
#[inline]
#[cfg(feature = "alloc")]
fn slice_ast_size<T: AstSize>(slice: &[T]) -> MaybeOverflow<usize> {
    let mut acc = MaybeOverflow::Contained(0);
    let mut scrutinee = slice;
    loop {
        match *scrutinee {
            [] => return acc,
            [ref head, ref tail @ ..] => {
                acc = acc.plus(1).plus_self(head.ast_size());
                if matches!(acc, MaybeOverflow::Overflow) {
                    // "Short-circuit":
                    return MaybeOverflow::Overflow;
                }
                scrutinee = tail;
            }
        }
    }
}

/// Value-size of a slice.
#[inline]
#[cfg(feature = "alloc")]
fn slice_value_size<T: ValueSize>(slice: &[T]) -> MaybeOverflow<usize> {
    let mut acc = MaybeOverflow::Contained(0);
    let mut scrutinee = slice;
    loop {
        match *scrutinee {
            [] => return acc,
            [ref head, ref tail @ ..] => {
                acc = acc.plus(1).plus_self(head.value_size());
                if matches!(acc, MaybeOverflow::Overflow) {
                    // "Short-circuit":
                    return MaybeOverflow::Overflow;
                }
                scrutinee = tail;
            }
        }
    }
}
