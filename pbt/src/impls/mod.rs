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
