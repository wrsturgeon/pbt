//! Fixed-point analysis for types that every generated value must contain.
//!
//! This module treats constructors as an AND/OR graph:
//! a type chooses one constructor, and a constructor contains all of its fields.
//! A type is unavoidable from `T` iff every finite value of type `T`
//! contains a subvalue of that type.

use {
    crate::hash::set,
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::hash::Hash,
    core::iter,
};

/// Collect all reachable vertices not already in `cache`.
#[inline]
fn collect_uncached<'ctors, Vertex, Variant, Constructors, Fields, FieldsOf>(
    vertex: Vertex,
    cache: &HashMap<Vertex, Arc<HashSet<Vertex>>>,
    constructors: &Constructors,
    fields_of: &FieldsOf,
    acc: &mut HashSet<Vertex>,
) where
    Constructors: Fn(Vertex) -> &'ctors [Variant],
    Fields: Iterator<Item = Vertex>,
    FieldsOf: Fn(&'ctors Variant) -> Fields,
    Variant: 'ctors,
    Vertex: Copy + Eq + Hash,
{
    if cache.contains_key(&vertex) || !acc.insert(vertex) {
        return;
    }

    let variants = constructors(vertex);
    for variant in variants {
        for field in fields_of(variant) {
            let () = collect_uncached(field, cache, constructors, fields_of, acc);
        }
    }
}

/// Compute and cache unavoidability for all uncached vertices reachable from `root`.
///
/// The caller supplies constructors and a field projection so this analysis stays generic over
/// the concrete reflection format. Results already present in `cache` are treated as final:
/// reachability collection stops at those vertices, and the fixed point refers to their cached
/// unavoidability sets directly.
///
/// The equation solved for each collected vertex is:
///
/// ```text
/// unavoidable(T) = {T} union the following:
///     intersection over constructors C of T:
///         union over fields F of C:
///             unavoidable(type of F)
/// ```
///
/// Solving the whole uncached reachable region at once handles cycles without needing an SCC
/// quotient graph.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub(crate) fn update<'ctors, Vertex, Variant, Constructors, Fields, FieldsOf>(
    root: Vertex,
    cache: &mut HashMap<Vertex, Arc<HashSet<Vertex>>>,
    constructors: &Constructors,
    fields_of: &FieldsOf,
) where
    Constructors: Fn(Vertex) -> &'ctors [Variant],
    Fields: Iterator<Item = Vertex>,
    FieldsOf: Fn(&'ctors Variant) -> Fields,
    Variant: 'ctors,
    Vertex: Copy + Eq + Hash,
{
    if cache.contains_key(&root) {
        return;
    }

    let mut domain = set();
    let () = collect_uncached(root, cache, constructors, fields_of, &mut domain);
    let mut solving: HashMap<Vertex, HashSet<Vertex>> = domain
        .iter()
        .map(|&vertex| (vertex, iter::once(vertex).collect()))
        .collect();

    'fixed_point: loop {
        let mut changed = false;

        #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
        for &vertex in &domain {
            let intersection = {
                let mut intersection: Option<HashSet<Vertex>> = None;
                for variant in constructors(vertex) {
                    let mut union = set::<Vertex>();
                    for dst in fields_of(variant) {
                        if let Some(unavoidable) = cache.get(&dst) {
                            let () = union.extend(&**unavoidable);
                        } else {
                            let () = union.extend(
                                solving
                                    .get(&dst)
                                    .expect("INTERNAL ERROR (`pbt`): unregistered type"),
                            );
                        }
                    }
                    if let Some(ref mut so_far) = intersection {
                        let () = so_far.retain(|unavoidable| union.contains(unavoidable));
                    } else {
                        intersection = Some(union);
                    }
                }
                if let Some(mut so_far) = intersection {
                    let _dup: bool = so_far.insert(vertex);
                    so_far
                } else {
                    iter::once(vertex).collect()
                }
            };
            let acc = solving
                .get_mut(&vertex)
                .expect("INTERNAL ERROR (`pbt`): witchcraft");
            for dst in intersection {
                changed |= acc.insert(dst);
            }
        }

        if !changed {
            break 'fixed_point;
        }
    }

    #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
    for (vertex, unavoidable) in solving {
        assert!(
            cache.insert(vertex, Arc::new(unavoidable)).is_none(),
            "INTERNAL ERROR (`pbt`): duplicate unavoidability result",
        );
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "Failing tests ought to panic.")]
    #![expect(clippy::unwrap_used, reason = "Failing tests ought to panic.")]

    use {
        super::*,
        crate::{hash::map, multiset::Multiset},
        pretty_assertions::assert_eq,
    };

    type AdtGraph = HashMap<u8, Arc<[Multiset<u8>]>>;

    fn adt(variants: &[&[u8]]) -> Arc<[Multiset<u8>]> {
        variants
            .iter()
            .copied()
            .map(|fields| fields.iter().copied().collect())
            .collect()
    }

    fn adt_graph<const N: usize>(entries: [(u8, Arc<[Multiset<u8>]>); N]) -> AdtGraph {
        let mut graph = map();
        for (vertex, adt) in entries {
            assert_eq!(
                graph.insert(vertex, adt),
                None,
                "test ADT graph has a duplicate vertex",
            );
        }
        graph
    }

    fn cached_unavoidables(cache: &HashMap<u8, Arc<HashSet<u8>>>, vertex: u8) -> HashSet<u8> {
        cache.get(&vertex).unwrap().as_ref().clone()
    }

    fn fields_of_multiset(variant: &Multiset<u8>) -> impl Iterator<Item = u8> {
        variant.iter_dedup().copied()
    }

    fn update_unavoidables_from(vertices: &AdtGraph, root: u8) -> HashMap<u8, Arc<HashSet<u8>>> {
        let mut cache = map();
        let () = update(
            root,
            &mut cache,
            &|i| {
                &**vertices
                    .get(&i)
                    .expect("test graph should contain requested constructors")
            },
            &fields_of_multiset,
        );
        cache
    }

    fn vertex_set(vertices: &[u8]) -> HashSet<u8> {
        let mut set = set();
        for vertex in vertices {
            let _: bool = set.insert(*vertex);
        }
        set
    }

    #[test]
    fn diamond_keeps_only_common_branch_dependencies() {
        // 1 = 2 | 3; 2 = 4; 3 = 4; 4 = leaf.
        let vertices = adt_graph([
            (1, adt(&[&[2], &[3]])),
            (2, adt(&[&[4]])),
            (3, adt(&[&[4]])),
            (4, adt(&[&[]])),
        ]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 4]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[2, 4]));
        assert_eq!(cached_unavoidables(&cache, 3), vertex_set(&[3, 4]));
        assert_eq!(cached_unavoidables(&cache, 4), vertex_set(&[4]));
    }

    #[test]
    fn leaf_variant_escapes_recursive_field() {
        // 1 = leaf | 2; 2 = leaf.
        let vertices = adt_graph([(1, adt(&[&[], &[2]])), (2, adt(&[&[]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[2]));
    }

    #[test]
    fn mutual_recursion_with_escape_is_asymmetric() {
        // 1 = 2; 2 = leaf | 1.
        let vertices = adt_graph([(1, adt(&[&[2]])), (2, adt(&[&[], &[1]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 2]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[2]));
    }

    #[test]
    fn mutual_recursion_without_escape_is_symmetric() {
        // 1 = 2; 2 = 1.
        let vertices = adt_graph([(1, adt(&[&[2]])), (2, adt(&[&[1]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 2]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[1, 2]));
    }

    #[test]
    fn forced_external_dependency_propagates_around_cycle() {
        // 1 = (2, 3); 2 = 1; 3 = leaf.
        let vertices = adt_graph([(1, adt(&[&[2, 3]])), (2, adt(&[&[1]])), (3, adt(&[&[]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 2, 3]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[1, 2, 3]));
        assert_eq!(cached_unavoidables(&cache, 3), vertex_set(&[3]));
    }

    #[test]
    fn cached_dependency_is_reused_without_traversal() {
        // 1 = 2, and 2's result is already known.
        let vertices = adt_graph([(1, adt(&[&[2]]))]);
        let mut cache = map();
        let _: Option<_> = cache.insert(2, Arc::new(vertex_set(&[2, 9])));

        let () = update(
            1,
            &mut cache,
            &|i| {
                vertices
                    .get(&i)
                    .expect("test graph should contain requested constructors")
            },
            &fields_of_multiset,
        );

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 2, 9]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[2, 9]));
    }
}
