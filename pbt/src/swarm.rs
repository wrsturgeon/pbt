//! A masked view into a type's constructors,
//! partitioned into potential leaves and loops.

use {
    crate::{
        Pbt, fields,
        hash::map,
        instantiability,
        multiset::Multiset,
        reflection::{
            Constructor, Constructors, Erased, Parts, Uninstantiable, constructors_of,
            register_globally,
        },
        scc,
        size::Size,
        unavoidability,
        union_find::UnionFind,
    },
    ahash::{HashMap, HashSet},
    alloc::{collections::BTreeMap, sync::Arc},
    core::{any::TypeId, mem, num::NonZero},
    wyrand::WyRand,
};

/// A type's constructors, partitioned into potential leaves and loops,
/// i.e. whether a sub-term of type `Self` is *avoidable* or *reachable*.
#[non_exhaustive]
enum Affordances {
    /// Algebraic constructors partitioned by whether they can leave or stay inside recursion.
    Algebraic {
        /// Sorted list of indices of instantiable constructors
        /// for which a sub-term of type `Self` is *avoidable*.
        potential_leaves: Box<[Constructor]>,
        /// Sorted list of indices of instantiable constructors
        /// for which a sub-term of type `Self` is *reachable*.
        potential_loops: Box<[Constructor]>,
    },
    /// Literal generators enabled by this swarm.
    Literal {
        /// Opaque function pointers that generate values of this type.
        generators: Box<[fn(&mut WyRand) -> Erased]>,
    },
}

/// A masked view into a type's constructors,
/// partitioned into potential leaves and loops.
pub(crate) struct Swarm {
    /// A masked (partial) set of constructors for this type,
    /// partitioned into potential leaves and loops.
    affordances: HashMap<TypeId, Affordances>,
}

impl Affordances {
    /// Whether this type can transitively
    /// contain a field of its own type.
    ///
    /// Equivalently, whether this type can be arbitrarily large.
    #[inline]
    #[must_use]
    fn is_inductive(&self) -> bool {
        match *self {
            Self::Algebraic {
                ref potential_loops,
                ..
            } => !potential_loops.is_empty(),
            Self::Literal { .. } => false,
        }
    }
}

impl Swarm {
    /// Split this type's (instantiable and unmasked)
    /// constructors into potential leaves and loops.
    #[inline]
    fn affordances<T>(&self) -> &Affordances
    where
        T: 'static,
    {
        self.affordances_of(TypeId::of::<T>())
    }

    /// Split this type's (instantiable and unmasked)
    /// constructors into potential leaves and loops.
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    fn affordances_of(&self, ty: TypeId) -> &Affordances {
        self.affordances
            .get(&ty)
            .expect("INTERNAL ERROR (`pbt`): unregistered type")
    }

    /// Generate an arbitrary term of some type.
    #[inline]
    #[expect(
        clippy::panic,
        reason = "Internal invariants: violations should fail loudly."
    )]
    pub(crate) fn arbitrary<T>(&self, size: Size, prng: &mut WyRand) -> T
    where
        T: Pbt,
    {
        let (potential_leaves, potential_loops) = match *self.affordances::<T>() {
            Affordances::Algebraic {
                ref potential_leaves,
                ref potential_loops,
            } => (potential_leaves, potential_loops),
            Affordances::Literal { ref generators } => {
                #[expect(
                    clippy::expect_used,
                    reason = "Swarms for uninstantiable literal types are rejected during construction."
                )]
                let n = NonZero::new(generators.len()).expect(
                    "INTERNAL ERROR (`pbt`): swarm created for an uninstantiable literal type",
                );
                #[expect(
                    clippy::as_conversions,
                    clippy::cast_possible_truncation,
                    reason = "Intentional lossy sampling from a 64-bit PRNG into machine-word indices."
                )]
                let generator_index = prng.rand() as usize % n;
                // SAFETY: `%` above.
                let erased = unsafe { *generators.get_unchecked(generator_index) };
                // SAFETY: `Registration::register::<T>` erased this function pointer.
                let generate = unsafe {
                    mem::transmute::<fn(&mut WyRand) -> Erased, fn(&mut WyRand) -> T>(erased)
                };
                return generate(prng);
            }
        };

        let (ctors, n) = if let Some(n_loops) = NonZero::new(potential_loops.len())
            && size.should_recurse(prng)
        {
            (potential_loops.as_ref(), n_loops)
        } else if let Some(n_leaves) = NonZero::new(potential_leaves.len()) {
            (potential_leaves.as_ref(), n_leaves)
        } else {
            panic!("INTERNAL ERROR (`pbt`): swarm created for an uninstantiable type")
        };

        #[expect(
            clippy::as_conversions,
            clippy::cast_possible_truncation,
            reason = "OK: `u64` is already huge"
        )]
        let ctor_index = prng.rand() as usize % n;
        // SAFETY: `%` above.
        let ctor = unsafe { ctors.get_unchecked(ctor_index) };

        let n_ind = self.count_inductive_fields(ctor.field_types());
        let sizes = size.partition(n_ind, prng);
        T::construct(Parts {
            fields: fields::Lazy {
                prng,
                sizes,
                swarm: self,
            },
            variant_index: Some(ctor.index),
        })
    }

    /// How many fields on this variant have inductive types?
    ///
    /// Two fields of the same type will count twice,
    /// since this is for partitioning a total size among fields.
    #[inline]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "if there were more than `usize::MAX` fields, they wouldn't have compiled"
    )]
    fn count_inductive_fields(&self, field_types: &Multiset<TypeId>) -> usize {
        let mut acc = 0_usize;
        for (&ty, count) in field_types.iter() {
            if self.affordances_of(ty).is_inductive() {
                acc += count.get();
            }
        }
        acc
    }

    /// Whether this type can transitively
    /// contain a field of its own type.
    ///
    /// Equivalently, whether this type can be arbitrarily large.
    #[inline]
    pub(crate) fn is_inductive<T>(&self) -> bool
    where
        T: 'static,
    {
        self.affordances::<T>().is_inductive()
    }

    /// Randomly disable a subset of variants for
    /// *all types used to construct this type*,
    /// including this type itself.
    ///
    /// See [the original swarm testing paper](https://dl.acm.org/doi/pdf/10.1145/2338965.2336763).
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    pub(crate) fn new<T>(
        prng: &mut WyRand,
        cache: &mut HashMap<BTreeMap<TypeId, Constructors<Erased>>, Option<Arc<Self>>>,
    ) -> Result<Arc<Self>, Uninstantiable>
    where
        T: Pbt,
    {
        let () = register_globally::<T>();
        let ty = TypeId::of::<T>();

        // Check if this type is instantiable *before* masking:
        if constructors_of(ty).is_empty() {
            return Err(Uninstantiable);
        }

        'rejection_sampling: loop {
            // Mask constructors at random (though preferring more variants over fewer variants):
            let mut naive_masked_constructors = BTreeMap::new();
            let () = mask_all_constructors_reachable_from(ty, &mut naive_masked_constructors, prng);
            if let Some(cached) = cache.get(&naive_masked_constructors) {
                match *cached {
                    None => continue 'rejection_sampling, // uninstantiable
                    Some(ref swarm) => return Ok(Arc::clone(swarm)),
                }
            }

            // Remove all uninstantiable variants:
            let mut masked_constructors = map();
            let () =
                instantiability::update(ty, &naive_masked_constructors, &mut masked_constructors);
            let () = masked_constructors.retain(|_, constructors| !constructors.is_empty());

            // If the original type is uninstantiable with these masks, try again:
            if masked_constructors
                .get(&ty)
                .is_none_or(Constructors::is_empty)
            {
                let _: &mut _ = cache.entry(naive_masked_constructors).or_insert(None);
                continue 'rejection_sampling;
            }

            // Compute strongly connected components, i.e. mutually inductive types:
            let mut scc_quotient_graph = UnionFind::new();
            let () = scc::update(
                ty,
                &|t| {
                    masked_constructors
                        .get(&t)
                        .expect("INTERNAL ERROR (`pbt`): unregistered type")
                        .algebraic()
                        .iter()
                        .flat_map(Constructor::dedup_fields)
                },
                &mut scc_quotient_graph,
            );

            let mut unavoidable = map();
            let () = unavoidability::update(
                ty,
                &mut unavoidable,
                &|t| {
                    masked_constructors
                        .get(&t)
                        .expect("INTERNAL ERROR (`pbt`): unregistered type")
                        .algebraic()
                },
                &|ctor| ctor.dedup_fields(),
            );

            let affordances = masked_constructors
                .into_iter()
                .map(|(t, ctors)| {
                    (
                        t,
                        affordances_of(t, ctors, &mut scc_quotient_graph, &unavoidable),
                    )
                })
                .collect();

            let arc = Arc::new(Self { affordances });
            let _: &mut _ = cache
                .entry(naive_masked_constructors)
                .or_insert(Some(Arc::clone(&arc)));

            return Ok(arc);
        }
    }
}

/// Given some total number of features,
/// how many should we enable?
///
/// This works better than enabling each individually,
/// since binomial distributions collapse very quickly.
#[inline]
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    reason = "OK: `u64` is already huge"
)]
#[expect(
    clippy::arithmetic_side_effects,
    reason = "if there were more than `usize::MAX` fields, they wouldn't have compiled"
)]
fn how_many_features_to_mask_out_of(n_total: usize, prng: &mut WyRand) -> usize {
    // Usually in programming we deal with [0, n),
    // but here we want [0, n], in which `n` itself is an option.
    // SAFETY: `n_total` is the length of an in-memory array,
    // so it cannot be `usize::MAX` while leaving enough memory for this subroutine.
    let inclusive = unsafe { NonZero::new_unchecked(n_total + 1) };

    // Weight "more features" more heavily than "fewer features":
    // specifically, "0 features" has weight 1, "1 feature" has weight 2,
    // etc., up to "all n features" with weight (n+1).
    'rejection_sampling: loop {
        // Sample two points in an `inclusive` by `inclusive` matrix:
        let y = prng.rand() as usize % inclusive;
        let x = prng.rand() as usize % inclusive;

        // Reject above the diagonal:
        if x > y {
            continue 'rejection_sampling;
        }

        return x;
    }
}

/// Pseudorandomly choose which of `n` features remain enabled.
#[inline]
#[expect(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::integer_division_remainder_used,
    reason = "OK: `u64` is already huge"
)]
fn mask_for(n_total: usize, prng: &mut WyRand) -> Vec<bool> {
    let n_to_mask = how_many_features_to_mask_out_of(n_total, prng);
    let mut mask = vec![true; n_total];
    for _ in 0..n_to_mask {
        'rejection_sampling: loop {
            let i = prng.rand() as usize % n_total;
            // SAFETY: `%` above
            let flip = unsafe { mask.get_unchecked_mut(i) };
            if *flip {
                *flip = false;
                break 'rejection_sampling;
            }
        }
    }
    mask
}

/// Build the masked affordance view for one type.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
fn affordances_of(
    ty: TypeId,
    constructors_of_ty: Constructors<Erased>,
    scc_quotient_graph: &mut UnionFind<TypeId>,
    unavoidable: &HashMap<TypeId, Arc<HashSet<TypeId>>>,
) -> Affordances {
    let constructors = match constructors_of_ty {
        Constructors::Algebraic(constructors) => constructors,
        Constructors::Literal { generators, .. } => {
            return Affordances::Literal {
                generators: generators.iter().copied().collect(),
            };
        }
    };

    let self_scc = scc_quotient_graph
        .root(ty)
        .expect("INTERNAL ERROR (`pbt`): SCC missing");

    let mut potential_leaves = vec![];
    let mut potential_loops = vec![];
    for constructor in &*constructors {
        let mut potential_leaf = true;
        let mut potential_loop = false;
        for field in constructor.dedup_fields() {
            if unavoidable
                .get(&field)
                .expect("INTERNAL ERROR (`pbt`): unavoidability missing")
                .contains(&ty)
            {
                potential_leaf = false;
            }
            if scc_quotient_graph
                .root(field)
                .expect("INTERNAL ERROR (`pbt`): SCC missing")
                == self_scc
            {
                potential_loop = true;
            }
        }
        if potential_leaf {
            let () = potential_leaves.push(constructor.clone());
        }
        // Both can coexist.
        if potential_loop {
            let () = potential_loops.push(constructor.clone());
        }
        debug_assert!(
            potential_leaf || potential_loop,
            "INTERNAL ERROR (`pbt`): constructor is neither a leaf nor a loop",
        );
    }

    Affordances::Algebraic {
        potential_leaves: potential_leaves.into_boxed_slice(),
        potential_loops: potential_loops.into_boxed_slice(),
    }
}

/// Run depth-first search, selecting constructors to mask for every type traversed.
#[inline]
fn mask_all_constructors_reachable_from(
    ty: TypeId,
    masked_constructors: &mut BTreeMap<TypeId, Constructors<Erased>>,
    prng: &mut WyRand,
) {
    if masked_constructors.contains_key(&ty) {
        return;
    }

    match constructors_of(ty) {
        Constructors::Algebraic(constructors) => {
            let mask = mask_for(constructors.len(), prng);
            let masked = constructors
                .iter()
                .zip(mask)
                .filter_map(|(constructor, enable)| enable.then_some(constructor.clone()))
                .collect();
            let _dup: Option<_> =
                masked_constructors.insert(ty, Constructors::Algebraic(Arc::clone(&masked)));

            for constructor in &*masked {
                for field_ty in constructor.dedup_fields() {
                    let () =
                        mask_all_constructors_reachable_from(field_ty, masked_constructors, prng);
                }
            }
        }
        Constructors::Literal {
            deserialize,
            generators,
            serialize,
            shrink,
        } => {
            let mask = mask_for(generators.len(), prng);
            let masked_generators = generators
                .iter()
                .zip(mask)
                .filter_map(|(&generate, enable)| enable.then_some(generate))
                .collect();
            let _dup: Option<_> = masked_constructors.insert(
                ty,
                Constructors::Literal {
                    deserialize,
                    generators: masked_generators,
                    serialize,
                    shrink,
                },
            );
        }
    }
}
