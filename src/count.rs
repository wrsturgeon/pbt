/// Cardinality of the minimal model of a type.
/// E.g. `!` is `Empty`, `()` is `Finite`, `usize` is `Finite`,
/// `Vec<()>` is `CountablyInfinite`, etc.
#[non_exhaustive]
pub enum Cardinality {
    /// No elements, i.e. an uninstantiable type:
    /// e.g. `!` or `core::convert::Infallible`.
    Empty,
    /// Finite cardinality: e.g. `u8`, `()`, etc.
    /// Note that, counterintuitively,
    /// we don't care about the cardinality of the type:
    /// types with one element or a billion are indistinguishable
    /// because in the limit, as we test more and more,
    /// countably infinite cardinalities will always be larger.
    Finite,
    /// At least countably infinitely many elements:
    /// e.g. inductive types like `Vec<()>`.
    Infinite,
}

/// Types "containing" a known number of terms*.
/// # asterisk
/// If you're a type theorist, the above statement will
/// read like an affront to all that is holy.
/// See e.g. <https://math.andrej.com/2013/08/28/the-elements-of-an-inductive-type>
/// for the subtle errors in this notion of a type.
/// However, this documentation is for Rust software development,
/// and this library's definition of "cardinality"
/// is already playing it fast and loose.
/// This is intentionally not a rigorous type-theoretic library;
/// instead, it's a practical approximation of type-theoretic insights
/// stuffed into Rust's non-dependent type system,
/// just as property-based testing is a practical approximation of
/// formal verification stuffed into the realm of computable functions.
pub trait Count {
    const CARDINALITY: Cardinality;
}

impl Cardinality {
    #[inline]
    #[must_use]
    pub const fn sum(self, other: Self) -> Self {
        match self {
            Self::Empty => other,
            Self::Infinite => Self::Infinite,
            Self::Finite => match other {
                Self::Empty | Self::Finite => Self::Finite,
                Self::Infinite => Self::Infinite,
            },
        }
    }
}
