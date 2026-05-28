//! The standard disjoint-set data structure
//! with metadata assigned to each set.

use {
    crate::hash::map,
    ahash::HashMap,
    core::{hash::Hash, mem, num::NonZero, ops::Deref},
};

/// The result of querying the root of a given element's set,
/// wihout any metadata.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub struct RootElement<K>(K);

/// The result of querying the root of a given element's set.
#[non_exhaustive]
#[derive(Clone, Debug)]
pub struct Root<K, V> {
    /// The total number of elements in this set.
    pub cardinality: NonZero<usize>,
    /// The arbitrary distinguished element that represents this entire set.
    pub element: RootElement<K>,
    /// User-defined metadata associated with this set as a whole.
    pub metadata: V,
}

/// The standard disjoint-set data structure
/// with metadata assigned to each set.
#[non_exhaustive]
pub struct UnionFind<K, V> {
    /// Either a parent or root metadata.
    upward: HashMap<K, Upward<K, V>>,
}

/// Either a parent or root metadata.
#[non_exhaustive]
#[derive(Clone, Debug, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub enum Upward<K, V> {
    /// This element is part of a larger set,
    /// and this parent is closer to its root.
    Parent {
        /// Another element in the same set, closer to its root.
        parent: K,
    },
    /// This element has been arbitrarily chosen
    /// as the distinguished element of this set.
    Root {
        /// The total number of elements in this set.
        cardinality: NonZero<usize>,
        /// User-defined metadata associated with this set as a whole.
        metadata: V,
    },
}

impl<K: PartialEq, V> PartialEq for Root<K, V> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.element == other.element
    }
}

impl<K: Eq, V> Eq for Root<K, V> {}

impl<K> Deref for RootElement<K> {
    type Target = K;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<K, V> Default for UnionFind<K, V> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<K, V> UnionFind<K, V> {
    /// Initialize an empty set of disjoint sets.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { upward: map() }
    }
}

#[expect(
    clippy::expect_used,
    clippy::panic,
    clippy::unwrap_in_result,
    reason = "For internal use only: invariant violations should fail loudly."
)]
impl<K, V> UnionFind<K, V>
where
    K: Copy + Eq + Hash,
    V: Clone,
{
    /// Insert a new element in a set of its own.
    ///
    /// # Panics
    ///
    /// If `k` is already present.
    #[inline]
    pub fn insert_singleton(&mut self, element: K, metadata: V) {
        if self
            .upward
            .insert(
                element,
                Upward::Root {
                    cardinality: const { NonZero::new(1).unwrap() },
                    metadata,
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
    /// `merge_metadata` should be commutative,
    /// since order is determined by pre-merge set cardinality.
    ///
    /// # Panics
    ///
    /// If either `lhs` or `rhs` is unregistered.
    #[inline]
    pub fn merge<MergeMetadata>(&mut self, lhs: K, rhs: K, merge_metadata: MergeMetadata)
    where
        MergeMetadata: FnOnce(V, V) -> V,
    {
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
            metadata: merge_metadata(lhs_root.metadata, rhs_root.metadata),
        };
    }

    /// Find the root of the set of which this key is a member,
    /// applying path shortening all the way up.
    ///
    /// # Panics
    ///
    /// If and only if internal invariants have already been violated.
    #[inline]
    pub fn root(&mut self, element: K) -> Option<Root<K, V>> {
        Some(match *self.upward.get(&element)? {
            Upward::Root {
                cardinality,
                ref metadata,
            } => Root {
                element: RootElement(element),
                cardinality,
                metadata: metadata.clone(),
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
        const SINGLETON: Upward<u8, ()> = Upward::Root {
            cardinality: NonZero::new(1).unwrap(),
            metadata: (),
        };

        let mut uf = UnionFind::new();

        // Start with {{1}, {2}, {3}, {4}, {5}}:
        for i in 1..=5_u8 {
            uf.insert_singleton(i, ());
        }
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&2), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&3), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&4), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&5), Some(&SINGLETON));

        // Merge 2 and 3 into {{1}, {2, 3}, {4}, {5}}:
        uf.merge(2, 3, |(), ()| ());
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(2).unwrap(),
                metadata: ()
            })
        );
        assert_eq!(uf.upward.get(&3), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&4), Some(&SINGLETON));
        assert_eq!(uf.upward.get(&5), Some(&SINGLETON));

        // Merge 4 and 5 into {{1}, {2, 3}, {4, 5}}:
        uf.merge(4, 5, |(), ()| ());
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(2).unwrap(),
                metadata: ()
            })
        );
        assert_eq!(uf.upward.get(&3), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(
            uf.upward.get(&4),
            Some(&Upward::Root {
                cardinality: NonZero::new(2).unwrap(),
                metadata: ()
            })
        );
        assert_eq!(uf.upward.get(&5), Some(&Upward::Parent { parent: 4 }));

        // Merge 3 and 5 into {{1}, {2, 3, 4, 5}}:
        uf.merge(3, 5, |(), ()| ());
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(4).unwrap(),
                metadata: ()
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
                metadata: (),
            })
        );
        assert_eq!(uf.upward.get(&1), Some(&SINGLETON));
        assert_eq!(
            uf.upward.get(&2),
            Some(&Upward::Root {
                cardinality: NonZero::new(4).unwrap(),
                metadata: ()
            })
        );
        assert_eq!(uf.upward.get(&3), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&4), Some(&Upward::Parent { parent: 2 }));
        assert_eq!(uf.upward.get(&5), Some(&Upward::Parent { parent: 2 })); // <-- here
    }
}
