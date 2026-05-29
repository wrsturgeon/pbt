//! A masked view into a type's constructors,
//! partitioned into potential leaves and loops.

use {
    crate::{
        Pbt, fields,
        hash::map,
        instantiability,
        multiset::Multiset,
        reflection::{
            Affordances, Constructor, Erased, Parts, Uninstantiable, Variant, constructors_of,
            register_globally,
        },
        scc,
        size::Size,
        unavoidability,
        union_find::UnionFind,
    },
    ahash::HashMap,
    alloc::{collections::BTreeMap, sync::Arc},
    core::{any::TypeId, num::NonZero, ptr},
    wyrand::WyRand,
};

/// A masked view into a type's constructors,
/// partitioned into potential leaves and loops.
pub(crate) struct Swarm {
    /// A masked (partial) set of constructors for this type,
    /// partitioned into potential leaves and loops.
    affordances: HashMap<TypeId, Affordances<Erased>>,
}

impl Swarm {
    /// Split this type's (instantiable and unmasked)
    /// constructors into potential leaves and loops.
    #[inline]
    fn affordances<T>(&self) -> &Affordances<T>
    where
        T: 'static,
    {
        let erased = self.affordances_of(TypeId::of::<T>());
        // SAFETY: `T` is only ever the codomain of a function pointer.
        unsafe {
            ptr::from_ref(erased)
                .cast::<Affordances<T>>()
                .as_ref_unchecked()
        }
    }

    /// Split this type's (instantiable and unmasked)
    /// constructors into potential leaves and loops.
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    fn affordances_of(&self, ty: TypeId) -> &Affordances<Erased> {
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
        let affordances = self.affordances::<T>();
        let (ctors, n) = if let Some(n_loops) = NonZero::new(affordances.potential_loops.len())
            && size.should_recurse(prng)
        {
            (&*affordances.potential_loops, n_loops)
        } else if let Some(n_leaves) = NonZero::new(affordances.potential_leaves.len()) {
            (&*affordances.potential_leaves, n_leaves)
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

        let field_types = match ctor.variant {
            Variant::Algebraic { ref field_types } => field_types,
            Variant::Literal { generator } => return generator(prng),
        };

        let n_ind = self.count_inductive_fields(field_types);
        let sizes = size.partition(n_ind, prng);
        T::construct(Parts {
            fields: fields::Lazy {
                prng,
                sizes,
                swarm: self,
            },
            variant_index: ctor.index,
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
        cache: &mut HashMap<BTreeMap<TypeId, Arc<[Constructor<Erased>]>>, Option<Arc<Self>>>,
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
            let () = instantiability::update(
                ty,
                &naive_masked_constructors,
                &mut masked_constructors,
                &Constructor::dedup_fields,
            );

            // If the original type is uninstantiable with these masks, try again:
            if masked_constructors
                .get(&ty)
                .is_none_or(|ctors| ctors.is_empty())
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
                },
                &|ctor| ctor.dedup_fields(),
            );

            let affordances = masked_constructors
                .into_iter()
                .map(|(t, ctors)| {
                    let self_scc = scc_quotient_graph
                        .root(t)
                        .expect("INTERNAL ERROR (`pbt`): SCC missing");

                    let mut potential_leaves = vec![];
                    let mut potential_loops = vec![];
                    for ctor in &*ctors {
                        let mut potential_leaf = true;
                        let mut potential_loop = false;
                        for field in ctor.dedup_fields() {
                            if unavoidable
                                .get(&field)
                                .expect("INTERNAL ERROR (`pbt`): unavoidability missing")
                                .contains(&t)
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
                            let () = potential_leaves.push(ctor.clone());
                        }
                        // Both can coexist!
                        if potential_loop {
                            let () = potential_loops.push(ctor.clone());
                        }
                        debug_assert!(
                            potential_leaf || potential_loop,
                            "INTERNAL ERROR (`pbt`): constructor is neither a leaf nor a loop",
                        );
                    }

                    (
                        t,
                        Affordances {
                            potential_leaves: potential_leaves.into_boxed_slice(),
                            potential_loops: potential_loops.into_boxed_slice(),
                        },
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

/// Run depth-first search, selecting constructors to mask for every type traversed.
#[inline]
#[expect(
    clippy::arithmetic_side_effects,
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::integer_division_remainder_used,
    reason = "OK: `u64` is already huge"
)]
fn mask_all_constructors_reachable_from(
    ty: TypeId,
    masked_constructors: &mut BTreeMap<TypeId, Arc<[Constructor<Erased>]>>,
    prng: &mut WyRand,
) {
    if masked_constructors.contains_key(&ty) {
        return;
    }

    let ctors = constructors_of(ty);

    let n_to_mask = how_many_features_to_mask_out_of(ctors.len(), prng);
    let mut mask = vec![true; ctors.len()];
    for _ in 0..n_to_mask {
        'rejection_sampling: loop {
            let i = prng.rand() as usize % ctors.len();
            // SAFETY: `%` above
            let flip = unsafe { mask.get_unchecked_mut(i) };
            if *flip {
                *flip = false;
                break 'rejection_sampling;
            }
        }
    }

    let arc = ctors
        .iter()
        .zip(mask)
        .filter_map(|(ctor, enable)| enable.then_some(ctor.clone()))
        .collect();
    let _dup: Option<_> = masked_constructors.insert(ty, Arc::clone(&arc));

    for ctor in &*arc {
        if let Variant::Algebraic { ref field_types } = ctor.variant {
            #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
            for &field_ty in field_types.iter_dedup() {
                let () = mask_all_constructors_reachable_from(field_ty, masked_constructors, prng);
            }
        }
    }
}
