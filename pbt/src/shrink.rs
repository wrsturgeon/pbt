use core::fmt;

/// Exhaustively generate all structurally inferred edge cases of this type.
pub trait Shrink: 'static + fmt::Debug + Sized {
    type Shrink: 'static + Iterator<Item = Self>;
    /// Exhaustively generate all structurally inferred edge cases of this type.
    fn shrink(&self) -> Self::Shrink;
}

#[inline]
pub fn shrink<E: Shrink>(full: &E) -> impl 'static + Iterator<Item = E> {
    full.shrink()
}
