use crate::{error, value_size::ValueSize};

/// Exhaustively generate all values
/// of this type of a given value size,
/// or report that the size was unreachable.
pub trait Exhaust: ValueSize + Sized {
    /// Exhaustively generate all values
    /// of this type of a given value size,
    /// or report that the size was unreachable.
    ///
    /// # Errors
    /// If the requested size was larger than
    /// that of the largest value of this type.
    fn exhaust(value_size: usize) -> Result<impl Iterator<Item = Self>, error::UnreachableSize>;
}

#[inline]
pub fn exhaust<E: Exhaust>() -> impl Iterator<Item = E> {
    (0..)
        .map_while(|value_size| <E as Exhaust>::exhaust(value_size).ok())
        .flatten()
}
