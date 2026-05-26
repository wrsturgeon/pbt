//! A masked view into a type's constructors,
//! partitioned into potential leaves and loops.

use {
    crate::reflection::{Affordances, Erased, LeavesAndLoops},
    ahash::HashMap,
    alloc::sync::Arc,
    core::{any::TypeId, num::NonZero},
    wyrand::WyRand,
};

/// A masked view into a type's constructors,
/// partitioned into potential leaves and loops.
pub struct Swarm<'full> {
    /// An immutable reference to the global map
    /// from types to their *full* set of constructors.
    full: &'full HashMap<TypeId, Affordances<Erased>>,
    /// A masked (partial) set of constructors for this type,
    /// partitioned into potential leaves and loops.
    masked: HashMap<TypeId, Affordances<Erased>>,
}

impl Swarm<'_> {
    /// A masked (partial) set of constructors for this type,
    /// partitioned into potential leaves and loops.
    #[inline]
    #[expect(
        clippy::expect_used,
        clippy::missing_panics_doc,
        reason = "For internal use only: invariant violations should fail loudly."
    )]
    pub fn affordances(&mut self, ty: TypeId, prng: &mut WyRand) -> &Affordances<Erased> {
        self.masked.entry(ty).or_insert_with(|| {
            let full = self
                .full
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): unregistered type during generation");
            let constructors = Arc::clone(&full.constructors);
            Affordances {
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_possible_truncation,
                    reason = "OK: `u64` is already huge"
                )]
                leaves_and_loops: {
                    let n_ctors = NonZero::new(constructors.len())
                        .expect("INTERNAL ERROR (`pbt`): uninstantiable type (TODO: accommodate)");
                    // Uniformly select the *number* of features to enable
                    // to avoid the near-50% collapse of binomial distributions.
                    let mut select_n = prng.rand() as usize % n_ctors;
                    // The above is on [0, n_ctors):
                    // note that it can never select all features,
                    // and it shouldn't ever select zero (uninstantiable without leaves).
                    // We'll shift everything up one by mandatorily selecting a leaf.

                    // If we selected more than half of the available constructors,
                    // consider `false` to be active and "disable" the complement:
                    let active = select_n <= (n_ctors.get() >> 1_u8);
                    if !active {
                        // SAFETY: By the modulo operation that set `select_n`.
                        select_n = unsafe { n_ctors.get().unchecked_sub(select_n) };
                        // The above is on (0, n_ctors]:
                        // since we re-enable one leaf,
                        // we should overshoot by one.
                    }

                    // Flip `select_n` features:
                    let mut enabled = vec![false; n_ctors.get()];
                    for _ in 0..select_n {
                        'rejection_sampling: loop {
                            let i = prng.rand() as usize % n_ctors;
                            // SAFETY: modulo `n_ctors`, which equals `enabled.len()` (set above)
                            let b_i = unsafe { enabled.get_unchecked_mut(i) };
                            if !*b_i {
                                *b_i = true;
                                break 'rejection_sampling;
                            }
                        }
                    }

                    // Then, additionally, enable a random leaf:
                    let n_leaves = NonZero::new(full.leaves_and_loops.potential_leaves.len())
                        .expect("INTERNAL ERROR (`pbt`): uninstantiable type (TODO: accommodate)");
                    let i_leaf = prng.rand() as usize % n_leaves;
                    let i_ctor =
                            // SAFETY: modulo `n_leaves`, which equals `...leaves.len()` (set above)
                        *unsafe { full.leaves_and_loops.potential_leaves.get_unchecked(i_leaf) };
                    let leaf_enabled_mut = enabled
                        .get_mut(i_ctor)
                        .expect("INTERNAL ERROR (`pbt`): invalid leaf index");
                    if *leaf_enabled_mut == active {
                        // Otherwise, enable a different feature at random
                        // (note that this is not the "flip" loop above,
                        // since it ensures the feature is `active`, not `true`):
                        'rejection_sampling: loop {
                            let i = prng.rand() as usize % n_ctors;
                            // SAFETY: modulo `n_ctors`, which equals `enabled.len()` (set above)
                            let b_i = unsafe { enabled.get_unchecked_mut(i) };
                            if *b_i != active {
                                *b_i = active;
                                break 'rejection_sampling;
                            }
                        }
                    } else {
                        *leaf_enabled_mut = active;
                    }

                    LeavesAndLoops {
                        potential_leaves: full
                            .leaves_and_loops
                            .potential_leaves
                            .iter()
                            .copied()
                            .filter(|&i| {
                                *enabled
                                    .get(i)
                                    .expect("INTERNAL ERROR (`pbt`): invalid leaf index")
                                    == active
                            })
                            .collect(),
                        potential_loops: full
                            .leaves_and_loops
                            .potential_loops
                            .iter()
                            .copied()
                            .filter(|&i| {
                                *enabled
                                    .get(i)
                                    .expect("INTERNAL ERROR (`pbt`): invalid loop index")
                                    == active
                            })
                            .collect(),
                    }
                },
                constructors,
            }
        })
    }
}
