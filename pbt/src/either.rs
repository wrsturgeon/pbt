//! One of two types, as an `enum` that tags which one is active.
//! Usually used to return one of two different iterator structures.

/// One of two types, as an `enum` that tags which one is active.
/// Usually used to return one of two different iterator structures.
#[expect(
    clippy::min_ident_chars,
    reason = "A and B make as much sense as anything else."
)]
#[expect(
    clippy::exhaustive_enums,
    reason = "Designed to hold one of exactly two variants."
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
