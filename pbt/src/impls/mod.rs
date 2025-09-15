//! Implementations for common types.

mod bool;
mod floats;
pub mod ints;
mod option;
mod tuples;
mod void;

#[cfg(feature = "alloc")]
mod alloc;

pub enum Either<A, B> {
    A(A),
    B(B),
}

#[expect(clippy::missing_trait_methods, reason = "would take years")]
impl<A: Iterator, B: Iterator<Item = A::Item>> Iterator for Either<A, B> {
    type Item = A::Item;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match *self {
            Self::A(ref mut a) => a.next(),
            Self::B(ref mut b) => b.next(),
        }
    }
}

/*
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
*/
