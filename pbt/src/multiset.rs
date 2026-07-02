//! A multiset/bag: an unordered collection tracking the
//! total count for potential duplicates of each element.

use {
    crate::hash::map,
    ahash::HashMap,
    core::{hash::Hash, num::NonZero},
    std::collections::hash_map,
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
    counts: HashMap<T, NonZero<usize>>,
}

impl<T> Default for Multiset<T>
where
    T: Eq + Hash,
{
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Multiset<T>
where
    T: Eq + Hash,
{
    /// Insert one copy of `value` into this multiset.
    #[inline]
    pub fn insert(&mut self, value: T) {
        self.counts
            .entry(value)
            .and_modify(|count: &mut NonZero<usize>| {
                #[expect(
                    clippy::arithmetic_side_effects,
                    reason = "intentional (and extremely unlikely)"
                )]
                // SAFETY: If the increment didn't overflow (which would have panicked),
                // then the result is greater than zero because the pre-increment was at least zero.
                let incremented = unsafe { NonZero::new_unchecked(count.get() + 1) };
                *count = incremented;
            })
            .or_insert(const { NonZero::new(1).unwrap() });
    }

    /// Iterate over each distinct element and its count.
    #[inline]
    pub(crate) fn iter(&self) -> impl Iterator<Item = (&T, NonZero<usize>)> {
        self.counts.iter().map(|(k, &v)| (k, v))
    }

    /// Iterate over each distinct element, ignoring duplicate counts.
    #[inline]
    #[must_use]
    pub(crate) fn iter_dedup(&self) -> hash_map::Keys<'_, T, NonZero<usize>> {
        self.counts.keys()
    }

    /// Initialize an empty multiset.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { counts: map() }
    }

    /// Remove exactly one element arbitrarily.
    #[inline]
    #[must_use]
    pub(crate) fn pop(&mut self) -> Option<T>
    where
        T: Clone,
    {
        let k = self.iter_dedup().next()?.clone();
        let () = self.remove(&k);
        Some(k)
    }

    /// Remove exactly one element.
    #[inline]
    pub(crate) fn remove(&mut self, t: &T) {
        let Some(n) = self.counts.get_mut(t) else {
            return;
        };
        // SAFETY: Nonzero.
        let Some(decremented) = NonZero::new(unsafe { n.get().unchecked_sub(1) }) else {
            let _: Option<_> = self.counts.remove(t);
            return;
        };
        *n = decremented;
    }

    /// The total number of elements, counting duplicates.
    #[inline]
    pub(crate) fn total(&self) -> usize {
        self.iter().map(|(_, n)| n.get()).sum()
    }
}

impl<T> FromIterator<T> for Multiset<T>
where
    T: Eq + Hash,
{
    /// Collect values into a multiset, counting duplicate elements.
    #[inline]
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut acc = Self::new();
        for t in iter {
            acc.insert(t);
        }
        acc
    }
}

impl<T> FromIterator<(T, usize)> for Multiset<T>
where
    T: Eq + Hash,
{
    /// Collect values into a multiset, counting duplicate elements.
    #[inline]
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (T, usize)>,
    {
        Self {
            counts: iter
                .into_iter()
                .filter_map(|(k, v)| Some((k, NonZero::new(v)?)))
                .collect(),
        }
    }
}

impl<T> FromIterator<(T, NonZero<usize>)> for Multiset<T>
where
    T: Eq + Hash,
{
    /// Collect values into a multiset, counting duplicate elements.
    #[inline]
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (T, NonZero<usize>)>,
    {
        Self {
            counts: iter.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {super::*, core::iter, pretty_assertions::assert_eq};

    #[test]
    fn insert_42_42_42() {
        let mut acc = Multiset::<usize>::new();
        let () = acc.insert(42);
        let () = acc.insert(42);
        let () = acc.insert(42);
        assert_eq!(
            acc,
            Multiset {
                counts: iter::once((42, const { NonZero::new(3).unwrap() })).collect()
            }
        );
        assert_eq!(acc.total(), 3);
    }

    #[test]
    fn zero_removes_key() {
        let mut acc = Multiset::<usize>::new();
        let () = acc.insert(42);
        let () = acc.remove(&42);
        assert_eq!(acc, Multiset { counts: map() });
        assert_eq!(acc.total(), 0);
    }
}
