//! Tagged union of two types.

#![expect(clippy::min_ident_chars, reason = "`A` and `B` are clear.")]

/// Tagged union of two types.
#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum Either<A, B> {
    /// The first of two types.
    A(A),
    /// The second of two types.
    B(B),
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
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
