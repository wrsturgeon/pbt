use core::fmt;

/// Exhaustively generate all structurally inferred edge cases of this type.
pub trait EdgeCases: 'static + fmt::Debug + Sized {
    type EdgeCases: 'static + Iterator<Item = Self>;
    /// Exhaustively generate all structurally inferred edge cases of this type.
    fn edge_cases() -> Self::EdgeCases;
}

#[inline]
pub fn edge_cases<E: EdgeCases>() -> impl 'static + Iterator<Item = E> {
    E::edge_cases()
}
