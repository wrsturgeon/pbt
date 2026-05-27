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
    clippy::missing_panics_doc,
    reason = "For internal use only: invariant violations should fail loudly."
)]
#[expect(clippy::implicit_hasher, reason = "all in on `ahash`")]
#[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
pub fn update_instantiability<'fields, Vertex, Variant, Fields, FieldsOfVariant>(
    naive: &'fields HashMap<Vertex, Arc<[Variant]>>,
    constructors: &mut HashMap<Vertex, Arc<[Variant]>>,
    fields_of_variant: &FieldsOfVariant,
) where
    Variant: 'fields + Clone,
    Vertex: 'fields + Copy + Eq + Hash,
    Fields: Iterator<Item = &'fields Vertex>,
    FieldsOfVariant: Fn(&'fields Variant) -> Fields,
{
    let mut masks: HashMap<Vertex, (bool, Box<[bool]>)> = map();
    for (&ty, variants) in naive {
        if !constructors.contains_key(&ty) {
            let _: Option<_> =
                masks.insert(ty, (false, iter::repeat_n(false, variants.len()).collect()));
        }
    }

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
                    if let Some(field_ctors) = constructors.get(field) {
                        !field_ctors.is_empty()
                    } else {
                        masks
                            .get(field)
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

    for (&ty, variants) in naive {
        let _: &mut _ = constructors.entry(ty).or_insert_with(|| -> Arc<[Variant]> {
            let variant_mask: &[bool] = &masks
                .get(&ty)
                .expect("INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis")
                .1;
            debug_assert_eq!(
                variant_mask.len(),
                variants.len(),
                "INTERNAL ERROR (`pbt`): variant size mismatch during instantiability analysis",
            );
            variants
                .iter()
                .zip(variant_mask)
                .filter_map(|(variant, &enabled)| enabled.then_some(variant))
                .cloned()
                .collect()
        });
    }
}
