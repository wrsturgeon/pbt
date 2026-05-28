//! The standard disjoint-set data structure.

use {
    crate::hash::map,
    ahash::HashMap,
    core::{hash::Hash, mem, num::NonZero, ops::Deref},
};

/// The distinguished element returned when querying a set's root.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub(crate) struct RootElement<Element>(Element);

/// The result of querying the root of a given element's set.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub(crate) struct Root<Element> {
    /// The total number of elements in this set.
    pub(crate) cardinality: NonZero<usize>,
    /// The arbitrary distinguished element that represents this entire set.
    pub(crate) element: RootElement<Element>,
}

/// The standard disjoint-set data structure.
#[non_exhaustive]
pub(crate) struct UnionFind<Element> {
    /// Either a parent or root cardinality.
    upward: HashMap<Element, Upward<Element>>,
}

/// Either a parent or root cardinality.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, Ord, Hash, PartialEq, PartialOrd)]
enum Upward<Element> {
    /// This element is part of a larger set,
    /// and this parent is closer to its root.
    Parent {
        /// Another element in the same set, closer to its root.
        parent: Element,
    },
    /// This element has been arbitrarily chosen
    /// as the distinguished element of this set.
    Root {
        /// The total number of elements in this set.
        cardinality: NonZero<usize>,
    },
}

impl<Element: PartialEq> PartialEq for Root<Element> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.element == other.element
    }
}

impl<Element: Eq> Eq for Root<Element> {}

impl<Element> Deref for RootElement<Element> {
    type Target = Element;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Element> Default for UnionFind<Element> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<Element> UnionFind<Element> {
    /// Initialize an empty set of disjoint sets.
    #[inline]
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self { upward: map() }
    }
}

#[expect(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_in_result,
    reason = "Internal invariants: violations should fail loudly."
)]
impl<Element> UnionFind<Element>
where
    Element: Copy + Eq + Hash,
{
    /// Insert a new element in a set of its own.
    ///
    /// # Panics
    ///
    /// If `k` is already present.
    #[inline]
    pub(crate) fn insert_singleton(&mut self, element: Element) {
        if self
            .upward
            .insert(
                element,
                Upward::Root {
                    cardinality: const { NonZero::new(1).unwrap() },
                },
            )
            .is_some()
        {
            panic!("INTERNAL ERROR (`pbt`): attempting to insert an already-present singleton")
        }
    }

    /// Merge two sets into their union.
    ///
    /// Each set is identified by an arbitrary member,
    /// not necessarily the root.
    ///
    /// # Panics
    ///
    /// If either `lhs` or `rhs` is unregistered.
    #[inline]
    pub(crate) fn merge(&mut self, lhs: Element, rhs: Element) {
        let mut lhs_root = self
            .root(lhs)
            .expect("INTERNAL ERROR (`pbt`): merging unregistered Union-Find entry");
        let mut rhs_root = self
            .root(rhs)
            .expect("INTERNAL ERROR (`pbt`): merging unregistered Union-Find entry");

        // Check if these elements are already in the same set:
        if lhs_root.element == rhs_root.element {
            return;
        }

        // Require that the larger set is on the left, w.l.o.g.:
        if rhs_root.cardinality > lhs_root.cardinality {
            let () = mem::swap(&mut lhs_root, &mut rhs_root);
        }

        // Point the smaller set at the larger set:
        let rhs_update = self
            .upward
            .get_mut(&rhs_root.element)
            .expect("INTERNAL ERROR (`pbt`): Union-Find entry erased");
        *rhs_update = Upward::Parent {
            parent: *lhs_root.element,
        };

        let lhs_update = self
            .upward
            .get_mut(&lhs_root.element)
            .expect("INTERNAL ERROR (`pbt`): Union-Find entry erased");
        *lhs_update = Upward::Root {
            // SAFETY: Already limited by the in-memory size of the hash map;
            // `usize`s can't overflow without exceeding available memory.
            #[expect(clippy::multiple_unsafe_ops_per_block, reason = "logically connected")]
            cardinality: unsafe {
                NonZero::new_unchecked(
                    lhs_root
                        .cardinality
                        .get()
                        .unchecked_add(rhs_root.cardinality.get()),
                )
            },
        };
    }

    /// Find the root of the set of which this key is a member,
    /// applying path shortening all the way up.
    ///
    /// # Panics
    ///
    /// If and only if internal invariants have already been violated.
    #[inline]
    pub(crate) fn root(&mut self, element: Element) -> Option<Root<Element>> {
        Some(match *self.upward.get(&element)? {
            Upward::Root { cardinality } => Root {
                element: RootElement(element),
                cardinality,
            },
            Upward::Parent { parent } => {
                let root = self
                    .root(parent)
                    .expect("INTERNAL ERROR (`pbt`): Union-Find parent is unregistered");
                let shorten = self
                    .upward
                    .get_mut(&element)
                    .expect("INTERNAL ERROR (`pbt`): Union-Find entry erased");
                *shorten = Upward::Parent {
                    parent: *root.element,
                };
                root
            }
        })
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "Failing tests ought to panic.")]

    use {super::*, pretty_assertions::assert_eq};

    #[test]
    #[expect(
        clippy::cognitive_complexity,
        reason = "[don draper voice] that's what the comments are for!"
    )]
    fn union_find_12345() {
        const SINGLETON: Upward<u8> = Upward::Root {
            cardinality: NonZero::new(1).unwrap(),
        };

        let mut uf = UnionFind::new();

        // Start with {{1}, {2}, {3}, {4}, {5}}:
        for i in 1..=5_u8 {
            uf.insert_singleton(i);
        }
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&2), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&3), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&4), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&5), Some(&SINGLETON));

        // Merge 2 and 3 into {{1}, {2, 3}, {4}, {5}}:
        uf.merge(2, 3);
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(2).unwrap(),
            })
        );
        assert_eq!(uf.upward.get(&3), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&4), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&5), Some(&SINGLETON));

        // Merge 4 and 5 into {{1}, {2, 3}, {4, 5}}:
        uf.merge(4, 5);
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(2).unwrap(),
            })
        );
        assert_eq!(uf.upward.get(&3), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(
            uf.upward.get(&4),
            Some(&Upward::Root {
                cardinality: NonZero::new(2).unwrap(),
            })
        );
        assert_eq!(uf.upward.get(&5), Some(&Upward::Parent { parent: 4 }));

        // Merge 3 and 5 into {{1}, {2, 3, 4, 5}}:
        uf.merge(3, 5);
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(4).unwrap(),
            })
        );
        assert_eq!(uf.upward.get(&3), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&4), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&5), Some(&Upward::Parent { parent: 4 })); // <-- suboptimal

        // Path shortening during lookup, so 5's parent becomes 2:
        assert_eq!(
            uf.root(5),
            Some(Root {
                cardinality: NonZero::new(4).unwrap(),
                element: RootElement(2),
            })
        );
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(4).unwrap(),
            })
        );
        assert_eq!(uf.upward.get(&3), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&4), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&5), Some(&Upward::Parent { parent: 2 })); // <-- here
    }
}
