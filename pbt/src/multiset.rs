use {
    core::{cmp, num::NonZero},
    std::collections::{BTreeMap, BTreeSet, btree_map},
};

/// One, as a non-zero integer. Stupid but efficient.
const ONE: NonZero<usize> = NonZero::new(1).unwrap();

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Multiset<T: Eq + Ord> {
    /// How many of each distinct element are in the bag?
    count: BTreeMap<T, NonZero<usize>>,
}

impl<T: Eq + Ord> Multiset<T> {
    #[inline]
    #[must_use]
    pub fn count(&self, element: &T) -> Option<NonZero<usize>> {
        self.count.get(element).copied()
    }

    #[inline]
    #[must_use]
    pub fn erase_counts(&self) -> BTreeSet<T>
    where
        T: Clone,
    {
        let mut acc = BTreeSet::new();
        for element in self.count.keys() {
            let _: bool = acc.insert(element.clone());
        }
        acc
    }

    /// Insert a single element, noting that
    /// this will increment that element's count if others are already present.
    /// # Panics
    /// If the total for that element (after insertion) would exceed `usize::MAX`.
    #[inline]
    pub fn insert(&mut self, element: T) -> NonZero<usize> {
        self.insert_n(element, ONE)
    }

    /// Insert an arbitrary (positive) quantity of a certain element.
    /// # Panics
    /// If the total for that element (after insertion) would exceed `usize::MAX`.
    #[inline]
    pub fn insert_n(&mut self, element: T, n: NonZero<usize>) -> NonZero<usize> {
        // `mut`, so TOCTOU is a non-issue
        let count = if let Some(count) = self.count(&element) {
            #[expect(clippy::expect_used, reason = "extremely unlikely")]
            count
                .checked_add(n.get())
                .expect("count for a single `Multiset` element exceeded `usize::MAX`")
        } else {
            n // Count::Finite(n)
        };
        let _: Option<_> = self.count.insert(element, count);
        count
    }

    #[inline]
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self
    where
        T: Clone,
    {
        let mut acc = Self::new();
        for (element, &count) in &self.count {
            if let Some(other_count) = other.count(element) {
                let count = count.min(other_count);
                let _: Option<_> = acc.count.insert(element.clone(), count);
            }
        }
        acc
    }

    /// Check whether this multiset is entirely contained in another multiset.
    /// If this is not a subset of `other`, this function returns `None`;
    /// if this function is a subset, this function returns `Some(strict)`,
    /// where `strict` is `false` iff `self` and `other` are *precisely equal*.
    #[inline]
    #[must_use]
    pub fn is_subset_of(&self, other: &Self) -> Option<bool> {
        // diferentiate equality from *strict* subset-ness:
        let mut strict = false; // set when `other` has *more* of some element than `self`

        for (ty, count) in self.iter() {
            match count.cmp(&other.count(ty)?) {
                cmp::Ordering::Greater => return None,
                cmp::Ordering::Equal => {}
                cmp::Ordering::Less => strict = true,
            }
        }
        Some(strict)
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&T, NonZero<usize>)> {
        self.count.iter().map(|(element, &count)| (element, count))
    }

    #[inline]
    #[must_use]
    pub fn new() -> Self {
        Self {
            count: BTreeMap::new(),
        }
    }

    /// The total sum of each element's count.
    /// Not cached because it's very rarely used;
    /// plus, this keeps the implementation simple enough to check.
    #[inline]
    #[must_use]
    pub fn total_count(&self) -> usize {
        self.count.iter().map(|(_, &n)| n.get()).sum()
    }

    #[inline]
    #[must_use]
    pub fn union(&self, other: &Self) -> BTreeSet<T>
    where
        T: Clone,
    {
        let mut acc = BTreeSet::new();
        for element in self.count.keys() {
            let _: bool = acc.insert(element.clone());
        }
        for element in other.count.keys() {
            let _: bool = acc.insert(element.clone());
        }
        acc
    }
}

impl<T: Eq + Ord> Default for Multiset<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<T: Eq + Ord> IntoIterator for Multiset<T> {
    type IntoIter = btree_map::IntoIter<T, NonZero<usize>>;
    type Item = (T, NonZero<usize>);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.count.into_iter()
    }
}

impl<T: Eq + Ord> FromIterator<T> for Multiset<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut acc = Self::new();
        for element in iter {
            let _: NonZero<usize> = acc.insert(element);
        }
        acc
    }
}
