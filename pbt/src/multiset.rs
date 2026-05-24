//! A multiset/bag: an unordered collection tracking the
//! total count for potential duplicates of each element.

use {
    ahash::HashMap,
    core::{hash::Hash, num::NonZero},
};

/// A multiset/bag: an unordered collection tracking the
/// total count for potential duplicates of each element.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Multiset<T>
where
    T: Eq + Hash,
{
    /// The count of potential duplicates of each element in the multiset.
    ///
    /// Note that the codomain is `NonZero<_>`, so
    /// `self.count.keys()` recovers the behavior of an ordinary set.
    pub counts: HashMap<T, NonZero<usize>>,
}
