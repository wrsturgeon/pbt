use {
    crate::{
        StronglyConnectedComponents,
        pbt::{
            Algebraic, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt, TypeFormer,
            push_arbitrary_field, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
    },
    ahash::{AHasher, HashMap, HashSet, RandomState},
    core::{
        cmp, fmt,
        hash::{Hash, Hasher},
        iter,
        num::NonZero,
    },
    std::collections::{BTreeSet, hash_map},
};

/// One, as a non-zero integer. Stupid but efficient.
const ONE: NonZero<usize> = NonZero::new(1).unwrap();

#[derive(Clone, Eq, PartialEq)]
pub struct Multiset<T: Eq + Hash> {
    /// How many of each distinct element are in the bag?
    count: HashMap<T, NonZero<usize>>,
}

impl<T: Eq + Hash> Multiset<T> {
    #[inline]
    #[must_use]
    pub fn count(&self, element: &T) -> Option<NonZero<usize>> {
        self.count.get(element).copied()
    }

    #[inline]
    #[must_use]
    pub fn erase_counts(&self) -> HashSet<T>
    where
        T: Clone,
    {
        let mut acc = HashSet::with_hasher(RandomState::new());
        for element in self.count.keys() {
            let _: bool = acc.insert(element.clone());
        }
        acc
    }

    #[inline]
    pub fn from_counts<I: IntoIterator<Item = (T, NonZero<usize>)>>(iter: I) -> Self {
        let mut count = HashMap::with_hasher(RandomState::new());
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
            count: HashMap::with_hasher(RandomState::new()),
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
    pub fn union(&self, other: &Self) -> HashSet<T>
    where
        T: Clone,
    {
        let mut acc = HashSet::with_hasher(RandomState::new());
        for element in self.count.keys() {
            let _: bool = acc.insert(element.clone());
        }
        for element in other.count.keys() {
            let _: bool = acc.insert(element.clone());
        }
        acc
    }
}

impl<T: Pbt + Hash> Pbt for Multiset<T> {
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
                arbitrary_fields: |prng, mut sizes| {
                    let mut fields = TermsOfVariousTypes::new();
                    push_arbitrary_field::<Vec<T>>(&mut fields, &mut sizes, prng)?;
                    Ok(fields)
                },
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
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(self.count.visit_deep())
    }
}

impl<T: fmt::Debug + Eq + Hash> fmt::Debug for Multiset<T> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.count, f)
    }
}

impl<T: Eq + Hash> Default for Multiset<T> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl<T: Eq + Hash> Hash for Multiset<T> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut xor = 0;
        for kv in &self.count {
            let mut hasher = AHasher::default();
            let () = kv.hash(&mut hasher);
            xor ^= hasher.finish();
        }
        xor.hash(state)
    }
}

impl<T: Eq + Hash> IntoIterator for Multiset<T> {
    type IntoIter = hash_map::IntoIter<T, NonZero<usize>>;
    type Item = (T, NonZero<usize>);

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.count.into_iter()
    }
}

impl<T: Eq + Hash> FromIterator<T> for Multiset<T> {
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut acc = Self::new();
        for element in iter {
            let _: NonZero<usize> = acc.insert(element);
        }
        acc
    }
}
