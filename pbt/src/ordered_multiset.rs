use {
    crate::{
        StronglyConnectedComponents,
        pbt::{
            Algebraic, ArbitraryFn, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt,
            TypeFormer, arbitrary_field, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
    },
    alloc::collections::{BTreeMap, BTreeSet, btree_map},
    core::{cmp, fmt, iter, num::NonZero},
};

/// One, as a non-zero integer. Stupid but efficient.
const ONE: NonZero<usize> = NonZero::new(1).unwrap();

/// A finite bag of ordered values, represented as element counts.
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Multiset<T: Ord> {
    /// How many of each distinct element are in the bag?
    count: BTreeMap<T, NonZero<usize>>,
}

impl<T: Ord> Multiset<T> {
    /// Return the count for `element`, if present.
    #[inline]
    #[must_use]
    pub fn count(&self, element: &T) -> Option<NonZero<usize>> {
        self.count.get(element).copied()
    }

    /// Return the set of elements, discarding their counts.
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

    /// Build a multiset from explicit non-zero counts.
    #[inline]
    pub fn from_counts<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = (T, NonZero<usize>)>,
    {
        let mut count = BTreeMap::new();
        for (element, n) in iter {
            let _: Option<NonZero<usize>> = count.insert(element, n);
        }
        Self { count }
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

    /// Return the multiset intersection, keeping the minimum count for shared elements.
    #[inline]
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self
    where
        T: Clone,
    {
        let mut acc = Self::new();
        for (element, &lhs_count) in &self.count {
            if let Some(other_count) = other.count(element) {
                let count = lhs_count.min(other_count);
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

    /// Iterate over distinct elements and their counts.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&T, NonZero<usize>)> {
        self.count.iter().map(|(element, &count)| (element, count))
    }

    /// Create an empty multiset.
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

    /// Return the set of elements that appear in either multiset, discarding counts.
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

impl<T: Ord + Pbt> Pbt for Multiset<T> {
    #[inline]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        sccs: &mut StronglyConnectedComponents,
    ) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<Vec<T>>(visited.clone(), sccs);
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            introduction_rules: vec![IntroductionRule {
                arbitrary: ArbitraryFn::new(|prng, mut sizes| {
                    let arbitrarily_ordered = arbitrary_field::<Vec<T>>(&mut sizes, prng)?;
                    Ok(Some(arbitrarily_ordered.into_iter().collect()))
                }),
                call: CtorFn::new(|terms| {
                    let arbitrarily_ordered: Vec<T> = terms.must_pop();
                    Some(arbitrarily_ordered.into_iter().collect())
                }),
                immediate_dependencies: iter::once(type_of::<Vec<T>>()).collect(),
            }],
            elimination_rule: ElimFn::new(|Multiset { count }| {
                let mut fields = TermsOfVariousTypes::new();
                let mut arbitrarily_ordered: Vec<T> = vec![];
                for (t, n) in count {
                    for _ in 0..n.get() {
                        let () = arbitrarily_ordered.push(t.clone());
                    }
                }
                let () = fields.push::<Vec<T>>(arbitrarily_ordered);
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
        })
    }

    #[inline]
    fn visit_deep<V>(&self) -> impl Iterator<Item = V>
    where
        V: Pbt,
    {
        visit_self(self).chain(self.count.visit_deep())
    }
}

impl<T: fmt::Debug + Ord> fmt::Debug for Multiset<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.count, f)
    }
}

impl<T: Ord> IntoIterator for Multiset<T> {
    type IntoIter = btree_map::IntoIter<T, NonZero<usize>>;
    type Item = (T, NonZero<usize>);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.count.into_iter()
    }
}

impl<T: Ord> FromIterator<T> for Multiset<T> {
    #[inline]
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let mut acc = Self::new();
        for element in iter {
            let _: NonZero<usize> = acc.insert(element);
        }
        acc
    }
}
