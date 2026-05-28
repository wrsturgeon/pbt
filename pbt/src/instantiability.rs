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
pub fn update<'naive, Constructor, CtorOfVariant, Vertex, Variant, Fields, FieldsOfVariant>(
    naive: &'naive HashMap<Vertex, Arc<[Variant]>>,
    constructors: &mut HashMap<Vertex, Arc<[Constructor]>>,
    fields_of_variant: &FieldsOfVariant,
    ctor_of_variant: CtorOfVariant,
) where
    CtorOfVariant: Fn(usize, Variant) -> Constructor,
    Fields: Iterator<Item = Vertex>,
    FieldsOfVariant: Fn(&'naive Variant) -> Fields,
    Variant: Clone,
    Vertex: Copy + Eq + Hash,
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

    for (&ty, variants) in naive {
        let _: &mut _ = constructors
            .entry(ty)
            .or_insert_with(|| -> Arc<[Constructor]> {
                let variant_mask: &[bool] = &masks
                    .get(&ty)
                    .expect(
                        "INTERNAL ERROR (`pbt`): unregistered type during instantiability analysis",
                    )
                    .1;
                debug_assert_eq!(
                    variant_mask.len(),
                    variants.len(),
                    "INTERNAL ERROR (`pbt`): variant size mismatch during instantiability analysis",
                );
                variants
                    .iter()
                    .enumerate()
                    .zip(variant_mask)
                    .filter_map(|(variant, &enabled)| enabled.then_some(variant))
                    .map(|(i, v)| ctor_of_variant(i, v.clone()))
                    .collect()
            });
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "Failing tests ought to panic.")]

    use {super::*, core::slice, pretty_assertions::assert_eq};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct TestVariant {
        fields: Arc<[u8]>,
        name: &'static str,
    }

    fn constructor_names(
        constructors: &HashMap<u8, Arc<[(usize, TestVariant)]>>,
        ty: u8,
    ) -> Vec<&'static str> {
        constructors
            .get(&ty)
            .expect("test should have registered this type")
            .iter()
            .map(|&(_, ref variant)| variant.name)
            .collect()
    }

    fn fields_of_variant(variant: &TestVariant) -> iter::Copied<slice::Iter<'_, u8>> {
        variant.fields.iter().copied()
    }

    fn instantiable_constructors<const N: usize>(
        entries: [(u8, Arc<[TestVariant]>); N],
    ) -> HashMap<u8, Arc<[(usize, TestVariant)]>> {
        let naive = test_naive_graph(entries);
        let mut constructors = map();
        let () = update::<(usize, TestVariant), _, u8, TestVariant, _, _>(
            &naive,
            &mut constructors,
            &fields_of_variant,
            &|i: usize, v: TestVariant| (i, v),
        );
        constructors
    }

    fn test_graph<const N: usize>(
        entries: [(u8, Arc<[(usize, TestVariant)]>); N],
    ) -> HashMap<u8, Arc<[(usize, TestVariant)]>> {
        let mut graph = map();
        for (ty, variants) in entries {
            assert_eq!(
                graph.insert(ty, variants),
                None,
                "test graph has a duplicate type",
            );
        }
        graph
    }

    fn test_naive_graph<const N: usize>(
        entries: [(u8, Arc<[TestVariant]>); N],
    ) -> HashMap<u8, Arc<[TestVariant]>> {
        let mut graph = map();
        for (ty, variants) in entries {
            assert_eq!(
                graph.insert(ty, variants),
                None,
                "test graph has a duplicate type",
            );
        }
        graph
    }

    fn ctor<const N: usize>(
        index: usize,
        name: &'static str,
        fields: [u8; N],
    ) -> (usize, TestVariant) {
        (
            index,
            TestVariant {
                fields: Arc::from(fields),
                name,
            },
        )
    }

    fn variant<const N: usize>(name: &'static str, fields: [u8; N]) -> TestVariant {
        TestVariant {
            fields: Arc::from(fields),
            name,
        }
    }

    fn ctors<const N: usize>(variants: [(usize, TestVariant); N]) -> Arc<[(usize, TestVariant)]> {
        Arc::from(variants)
    }

    fn variants<const N: usize>(variants: [TestVariant; N]) -> Arc<[TestVariant]> {
        Arc::from(variants)
    }

    #[test]
    fn leaf_constructor_seeds_the_fixed_point() {
        // 1 = leaf.
        let constructors = instantiable_constructors([(1, variants([variant("one::Leaf", [])]))]);

        assert_eq!(constructor_names(&constructors, 1), vec!["one::Leaf"]);
    }

    #[test]
    fn uninstantiable_field_removes_only_the_variants_that_need_it() {
        // 1 = leaf | impossible(2); 2 = !.
        let constructors = instantiable_constructors([
            (
                1,
                variants([variant("one::Leaf", []), variant("one::Impossible", [2])]),
            ),
            (2, variants([])),
        ]);

        assert_eq!(constructor_names(&constructors, 1), vec!["one::Leaf"]);
        assert_eq!(
            constructor_names(&constructors, 2),
            Vec::<&'static str>::new()
        );
    }

    #[test]
    fn self_cycle_without_leaf_is_not_a_finite_term() {
        // 1 = loop(1).
        let constructors = instantiable_constructors([(1, variants([variant("one::Loop", [1])]))]);

        assert_eq!(
            constructor_names(&constructors, 1),
            Vec::<&'static str>::new()
        );
    }

    #[test]
    fn mutual_cycle_without_escape_is_not_a_finite_term() {
        // 1 = needs_2(2); 2 = needs_1(1).
        let constructors = instantiable_constructors([
            (1, variants([variant("one::NeedsTwo", [2])])),
            (2, variants([variant("two::NeedsOne", [1])])),
        ]);

        assert_eq!(
            constructor_names(&constructors, 1),
            Vec::<&'static str>::new()
        );
        assert_eq!(
            constructor_names(&constructors, 2),
            Vec::<&'static str>::new()
        );
    }

    #[test]
    fn mutual_cycle_with_one_escape_makes_the_whole_cycle_instantiable() {
        // 1 = needs_2(2); 2 = leaf | needs_1(1).
        let constructors = instantiable_constructors([
            (1, variants([variant("one::NeedsTwo", [2])])),
            (
                2,
                variants([variant("two::Leaf", []), variant("two::NeedsOne", [1])]),
            ),
        ]);

        assert_eq!(constructor_names(&constructors, 1), vec!["one::NeedsTwo"]);
        assert_eq!(
            constructor_names(&constructors, 2),
            vec!["two::Leaf", "two::NeedsOne"],
        );
    }

    #[test]
    fn cached_empty_constructors_are_not_instantiable() {
        // 1 = needs_cached_void(2), while a previous pass proved 2 = !.
        let naive = test_naive_graph([(1, variants([variant("one::NeedsCachedVoid", [2])]))]);
        let mut constructors = test_graph([(2, ctors([]))]);

        let () = update(&naive, &mut constructors, &fields_of_variant, |i, v| (i, v));

        assert_eq!(
            constructor_names(&constructors, 1),
            Vec::<&'static str>::new()
        );
        assert_eq!(
            constructor_names(&constructors, 2),
            Vec::<&'static str>::new()
        );
    }

    #[test]
    fn cached_nonempty_constructors_are_instantiable() {
        // 1 = needs_cached_leaf(2), while a previous pass proved 2 = leaf.
        let naive = test_naive_graph([(1, variants([variant("one::NeedsCachedLeaf", [2])]))]);
        let mut constructors = test_graph([(2, ctors([ctor(0, "two::Leaf", [])]))]);

        let () = update(&naive, &mut constructors, &fields_of_variant, |i, v| (i, v));

        assert_eq!(
            constructor_names(&constructors, 1),
            vec!["one::NeedsCachedLeaf"]
        );
        assert_eq!(constructor_names(&constructors, 2), vec!["two::Leaf"]);
    }
}
