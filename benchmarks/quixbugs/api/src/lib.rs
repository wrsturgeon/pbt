//! Common interface implemented by each property-based testing library.

/// Find and shrink a counterexample to a property over `T`.
pub trait Check<T> {
    /// Return a counterexample for which `property` is false.
    fn check(property: fn(&T) -> bool) -> Option<T>;
}

/// A library supporting every input type used by the initial `QuixBugs` benchmark.
pub trait Library:
    Check<String>
    + Check<(String, String)>
    + Check<usize>
    + Check<(usize, Vec<(usize, usize)>)>
    + Check<(usize, usize)>
    + Check<(usize, usize, usize)>
    + Check<Vec<usize>>
    + Check<(Vec<usize>, usize)>
{
}

impl<L> Library for L where
    L: Check<String>
        + Check<(String, String)>
        + Check<usize>
        + Check<(usize, Vec<(usize, usize)>)>
        + Check<(usize, usize)>
        + Check<(usize, usize, usize)>
        + Check<Vec<usize>>
        + Check<(Vec<usize>, usize)>
{
}
