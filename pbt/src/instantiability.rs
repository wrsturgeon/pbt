//! Graph-theoretic operations on abstract vertex/edge/variant types.
//!
//! Algebraic data type dependencies are modeled as a bipartite graph in which
//! types point to constructors and constructors point to types.
//! Each directed edge means "contains," i.e.
//! "has a field of this type" or "contains this variant."

use {
    crate::hash::map,
    ahash::HashMap,
    alloc::sync::Arc,
    core::{hash::Hash, iter},
};

/// Run depth-first search, masking everything in sight,
/// so we can incrementally un-mask until we reach a fixed point.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn mask_all_reachable<'naive, Fields, FieldsOfVariant, Variant, Vertex>(
    root: Vertex,
    naive: &'naive HashMap<Vertex, Arc<[Variant]>>,
    constructors: &mut HashMap<Vertex, Arc<[Variant]>>,
    fields_of_variant: &FieldsOfVariant,
    masks: &mut HashMap<Vertex, (bool, Box<[bool]>)>,
) where
    Fields: Iterator<Item = Vertex>,
    FieldsOfVariant: Fn(&'naive Variant) -> Fields,
    Vertex: Eq + Hash,
{
    if constructors.contains_key(&root) || masks.contains_key(&root) {
        return;
    }

    let variants = naive
        .get(&root)
        .expect("INTERNAL ERROR (`pbt`): unregistered type");
    let _: Option<_> = masks.insert(
        root,
        (false, iter::repeat_n(false, variants.len()).collect()),
    );
    for variant in &**variants {
        for field in fields_of_variant(variant) {
            let () = mask_all_reachable(field, naive, constructors, fields_of_variant, masks);
        }
    }
}

/// Run depth-first search, submitting each result we traverse.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn finalize_all_reachable<'naive, Fields, FieldsOfVariant, Variant, Vertex>(
    root: Vertex,
    naive: &'naive HashMap<Vertex, Arc<[Variant]>>,
    constructors: &mut HashMap<Vertex, Arc<[Variant]>>,
    fields_of_variant: &FieldsOfVariant,
    masks: &HashMap<Vertex, (bool, Box<[bool]>)>,
) where
    Fields: Iterator<Item = Vertex>,
    FieldsOfVariant: Fn(&'naive Variant) -> Fields,
    Variant: Clone,
    Vertex: Clone + Eq + Hash,
{
    if constructors.contains_key(&root) {
        return;
    }

    let (type_mask, ref variant_mask) = *masks
        .get(&root)
        .expect("INTERNAL ERROR (`pbt`): mask disappeared");
    if !type_mask {
        return;
    }

    let variants = &**naive
        .get(&root)
        .expect("INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis");
    debug_assert_eq!(
        variant_mask.len(),
        variants.len(),
        "INTERNAL ERROR (`pbt`): variant size mismatch during instantiability analysis",
    );

    let _: &mut _ = constructors
        .entry(root)
        .or_insert_with(|| -> Arc<[Variant]> {
            variants
                .iter()
                .zip(variant_mask)
                .filter_map(|(variant, enabled)| enabled.then_some(variant))
                .cloned()
                .collect()
        });

    for variant in variants
        .iter()
        .zip(variant_mask)
        .filter_map(|(variant, enabled)| enabled.then_some(variant))
    {
        for field in fields_of_variant(variant) {
            let () = finalize_all_reachable(field, naive, constructors, fields_of_variant, masks);
        }
    }
}

/// Incrementally computes the least fixed point of the following relation:
///
/// - Constructors are instantiable if all their fields' types are instantiable.
/// - Types are instantiable if any of their constructors are instantiable.
///
/// We specify the *least* fixed point to exclude
/// coinductive/infinitely-sized types like `struct Loop(Box<Self>)`.
///
/// A nice consequence is that a type's instantiability *after* this pass
/// is merely `!constructors.is_empty()`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
#[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
pub(crate) fn update<'naive, Fields, FieldsOfVariant, Variant, Vertex>(
    root: Vertex,
    naive: &'naive HashMap<Vertex, Arc<[Variant]>>,
    constructors: &mut HashMap<Vertex, Arc<[Variant]>>,
    fields_of_variant: &FieldsOfVariant,
) where
    Fields: Iterator<Item = Vertex>,
    FieldsOfVariant: Fn(&'naive Variant) -> Fields,
    Variant: Clone,
    Vertex: Copy + Eq + Hash,
{
    let mut masks: HashMap<Vertex, (bool, Box<[bool]>)> = map();
    let () = mask_all_reachable(root, naive, constructors, fields_of_variant, &mut masks);

    'fixed_point: loop {
        let mut changed = false;

        // 'types: for (&ty, &mut (ref mut type_mask, ref mut variant_mask)) in &mut masks {
        'types: for (&ty, naive_variants) in naive {
            if !masks.contains_key(&ty) {
                continue 'types;
            }
            'variants: for i in 0..naive_variants.len() {
                // SAFETY: Loop bounds above.
                let naive_variant = unsafe { naive_variants.get_unchecked(i) };
                let variant_masks: &[bool] = &masks
                    .get(&ty)
                    .expect("INTERNAL ERROR (`pbt`): mask disappeared")
                    .1;
                debug_assert_eq!(
                    variant_masks.len(),
                    naive_variants.len(),
                    "INTERNAL ERROR (`pbt`): variant size mismatch",
                );
                // SAFETY: Invariant, also checked above.
                if *unsafe { variant_masks.get_unchecked(i) } {
                    continue 'variants;
                }
                let instantiable = fields_of_variant(naive_variant).all(|field| {
                    if let Some(field_ctors) = constructors.get(&field) {
                        !field_ctors.is_empty()
                    } else {
                        masks
                            .get(&field)
                            .expect("INTERNAL ERROR (`pbt`): mask disappeared")
                            .0
                    }
                });
                if instantiable {
                    let variant_masks_mut: &mut [bool] = &mut masks
                        .get_mut(&ty)
                        .expect("INTERNAL ERROR (`pbt`): mask disappeared")
                        .1;
                    // SAFETY: Invariant, also checked above.
                    let variant_mask = unsafe { variant_masks_mut.get_unchecked_mut(i) };
                    *variant_mask = true;
                    changed = true;
                }
            }
            let (type_mask, ref variant_masks) = *masks
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): TOCTOU despite holding a reference");
            if type_mask {
                continue 'types;
            }
            if variant_masks.iter().any(|&instantiable| instantiable) {
                masks
                    .get_mut(&ty)
                    .expect("INTERNAL ERROR (`pbt`): TOCTOU despite holding a reference")
                    .0 = true;
                changed = true;
            }
        }

        if !changed {
            break 'fixed_point;
        }
    }

    let () = finalize_all_reachable(root, naive, constructors, fields_of_variant, &masks);
}
