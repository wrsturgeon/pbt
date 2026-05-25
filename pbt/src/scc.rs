//! Tarjan's strongly connected components algorithm.

use {
    crate::{
        hash::{map, set},
        multiset::Multiset,
        union_find::{RootElement, UnionFind},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::hash::Hash,
};

/// Metadata associated with one SCC in the quotient graph.
#[non_exhaustive]
pub struct QuotientVertex<Vertex> {
    /// All vertices in this strongly connected component.
    pub elements: HashSet<Vertex>,

    /// All fields of all types within an SCC (i.e. a mutually inductive set of types)
    /// that do not themselves belong to the SCC.
    ///
    /// Recall that fields of each individual type are directed edges,
    /// so directed edges out of an SCC are not a very well-defined concept,
    /// but they could be seen as representing "optional dependencies,"
    /// i.e. that there exists a generator path that contains a term of this type.
    pub immediately_reachable: HashSet<RootElement<Vertex>>,
}

/// Per-vertex bookkeeping for Tarjan's SCC algorithm.
#[non_exhaustive]
pub struct VertexBookkeeping {
    /// The arbitrary global DFS timestamp at which we visited this vertex.
    global_visit_index: usize,
    /// The smallest index of any node on the stack known to be
    /// reachable from `v` through `v`'s DFS subtree,
    /// including `v` itself. (Direct quote from <https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm#The_algorithm_in_pseudocode>.)
    low_link: usize,
    /// Whether this vertex is currently on the stack.
    on_stack: bool,
}

/// Run Tarjan's strongly connected components algorithm from the selected vertex.
///
/// See <https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm#The_algorithm_in_pseudocode>.
#[inline]
pub fn update_quotient_reachable_from<'edges, OutgoingEdges, Vertex>(
    vertex: Vertex,
    outgoing_edges: &OutgoingEdges,
    quotient: &mut UnionFind<Vertex, Arc<QuotientVertex<Vertex>>>,
) where
    OutgoingEdges: Fn(Vertex) -> &'edges HashSet<Vertex>,
    Vertex: 'edges + Copy + Eq + Hash,
{
    if quotient.root(vertex).is_some() {
        return; // SCC already discovered from some other type
    }

    let () = tarjan(
        vertex,
        outgoing_edges,
        quotient,
        &mut 0,
        &mut map(),
        &mut vec![],
    );
}

/// Compute all quotient roots reachable from `root`, including `root` itself.
///
/// The SCC quotient graph is a DAG by construction, and reachability is
/// reflexive by convention to simplify downstream reachability checks.
///
/// # Panics
///
/// If the quotient graph does not contain `root` or one of its reachable children.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::implicit_hasher,
    clippy::iter_over_hash_type,
    clippy::panic,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn reachable<Vertex>(
    cache: &mut HashMap<RootElement<Vertex>, Arc<HashSet<RootElement<Vertex>>>>,
    quotient: &mut UnionFind<Vertex, Arc<QuotientVertex<Vertex>>>,
    root: RootElement<Vertex>,
) -> Arc<HashSet<RootElement<Vertex>>>
where
    Vertex: Copy + Eq + Hash,
{
    if let Some(cached) = cache.get(&root) {
        return Arc::clone(cached);
    }

    let mut union = set();
    let newly_inserted = union.insert(root);
    debug_assert!(newly_inserted, "INTERNAL ERROR (`pbt`): witchcraft");

    for &child in &quotient
        .root(*root)
        .expect("INTERNAL ERROR (`pbt`): unregistered vertex during reachability analysis")
        .metadata
        .immediately_reachable
    {
        let () = union.extend(reachable(cache, quotient, child).iter());
    }

    let arc = Arc::new(union);
    let to_return = Arc::clone(&arc);
    if let Some(_dup) = cache.insert(root, arc) {
        panic!("INTERNAL ERROR (`pbt`): SCC quotient graph is cyclic")
    }
    to_return
}

/// Compute and cache unavoidability sets for the SCC containing `vertex`.
///
/// The caller supplies the ordinary vertex metadata map and two projections:
/// one from metadata to variants, and one from variants to field multisets.
/// This keeps the fixed-point algorithm generic over the reflection format.
///
/// # Panics
///
/// If the quotient graph does not contain a needed vertex,
/// or if the ordinary vertex metadata map is missing that vertex's metadata.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::implicit_hasher,
    clippy::iter_over_hash_type,
    clippy::panic,
    reason = "For internal use only: invariant violations should fail loudly."
)]
pub fn update_unavoidable<Vertex, Adt, Variant, VariantsOf, FieldsOf>(
    vertex: Vertex,
    cache: &mut HashMap<Vertex, Arc<HashSet<Vertex>>>,
    vertices: &HashMap<Vertex, Adt>,
    quotient: &mut UnionFind<Vertex, Arc<QuotientVertex<Vertex>>>,
    variants_of: &VariantsOf,
    fields_of: &FieldsOf,
) where
    Vertex: Copy + Eq + Hash,
    VariantsOf: Fn(&Adt) -> &[Variant],
    FieldsOf: Fn(&Variant) -> &Multiset<Vertex>,
{
    if cache.contains_key(&vertex) {
        return;
    }

    let scc_elements = quotient
        .root(vertex)
        .expect("INTERNAL ERROR (`pbt`): unregistered vertex during unavoidability analysis")
        .metadata
        .elements
        .clone();

    for &scc_element in &scc_elements {
        let adt = vertices
            .get(&scc_element)
            .expect("INTERNAL ERROR (`pbt`): unregistered vertex during unavoidability analysis");
        let variants: &[Variant] = variants_of(adt);
        for variant in variants {
            for field in fields_of(variant).counts.keys() {
                if !scc_elements.contains(field) {
                    let () = update_unavoidable(
                        *field,
                        cache,
                        vertices,
                        quotient,
                        variants_of,
                        fields_of,
                    );
                }
            }
        }
    }

    for (fixed_vertex, unavoidables) in
        fixed_point_unavoidable(vertices, cache, &scc_elements, variants_of, fields_of)
    {
        let arc = Arc::new(unavoidables);
        if let Some(_dup) = cache.insert(fixed_vertex, arc) {
            panic!("INTERNAL ERROR (`pbt`): duplicate unavoidability result")
        }
    }
}

/// Compute the least fixed point of unavoidability for one SCC.
#[inline]
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn fixed_point_unavoidable<Vertex, Adt, Variant, VariantsOf, FieldsOf>(
    vertices: &HashMap<Vertex, Adt>,
    cached: &HashMap<Vertex, Arc<HashSet<Vertex>>>,
    scc_elements: &HashSet<Vertex>,
    variants_of: &VariantsOf,
    fields_of: &FieldsOf,
) -> HashMap<Vertex, HashSet<Vertex>>
where
    Vertex: Copy + Eq + Hash,
    VariantsOf: Fn(&Adt) -> &[Variant],
    FieldsOf: Fn(&Variant) -> &Multiset<Vertex>,
{
    let mut scc_acc = map();

    #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
    for &scc_vertex in scc_elements {
        let mut just_self = set();
        let newly_inserted = just_self.insert(scc_vertex);
        debug_assert!(newly_inserted, "INTERNAL ERROR (`pbt`): witchcraft");
        assert!(
            scc_acc.insert(scc_vertex, just_self).is_none(),
            "INTERNAL ERROR (`pbt`): duplicate unavoidability entry",
        );
    }

    // Currently, this iteratively scans until a least fixed point is reached.
    // Other approaches may be more efficient as SCCs grow asymptotically large,
    // but mutually inductive groups of types in Rust are almost always tiny,
    // so readability and auditability are most important here. May change later.
    loop {
        let mut changed = false;

        #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
        for &scc_vertex in scc_elements {
            let adt = vertices.get(&scc_vertex).expect(
                "INTERNAL ERROR (`pbt`): unregistered vertex during unavoidability analysis",
            );
            let variant_slice: &[Variant] = variants_of(adt);
            let mut variants = variant_slice.iter();
            let mut next = match variants.next() {
                Some(variant) => variant_unavoidable::<Vertex, Variant, FieldsOf>(
                    variant, cached, &scc_acc, fields_of,
                ),
                None => set(),
            };
            for variant in variants {
                let variant_unavoidables = variant_unavoidable::<Vertex, Variant, FieldsOf>(
                    variant, cached, &scc_acc, fields_of,
                );
                next.retain(|candidate| variant_unavoidables.contains(candidate));
            }
            let _: bool = next.insert(scc_vertex);

            let slot = scc_acc
                .get_mut(&scc_vertex)
                .expect("INTERNAL ERROR (`pbt`): missing unavoidability entry");
            if *slot != next {
                *slot = next;
                changed = true;
            }
        }
        if !changed {
            return scc_acc;
        }
    }
}

/// Run Tarjan's strongly connected components algorithm from the selected vertex.
///
/// See <https://en.wikipedia.org/wiki/Tarjan%27s_strongly_connected_components_algorithm#The_algorithm_in_pseudocode>.
#[inline]
#[expect(
    clippy::arithmetic_side_effects,
    clippy::expect_used,
    clippy::panic,
    reason = "For internal use only: invariant violations should fail loudly."
)]
#[expect(clippy::too_many_lines, reason = "take it up with Tarjan")]
fn tarjan<'edges, OutgoingEdges, Vertex>(
    vertex: Vertex,
    outgoing_edges: &OutgoingEdges,
    quotient: &mut UnionFind<Vertex, Arc<QuotientVertex<Vertex>>>,
    global_visit_index: &mut usize,
    bookkeeping: &mut HashMap<Vertex, VertexBookkeeping>,
    stack: &mut Vec<Vertex>,
) where
    OutgoingEdges: Fn(Vertex) -> &'edges HashSet<Vertex>,
    Vertex: 'edges + Copy + Eq + Hash,
{
    macro_rules! get {
        ($e:expr) => {
            bookkeeping
                .get($e)
                .expect("INTERNAL ERROR (`pbt`): inconsistent SCC bookkeeping")
        };
    }

    macro_rules! get_mut {
        ($e:expr) => {
            bookkeeping
                .get_mut($e)
                .expect("INTERNAL ERROR (`pbt`): inconsistent SCC bookkeeping")
        };
    }

    macro_rules! root {
        ($e:expr) => {
            quotient
                .root($e)
                .expect("INTERNAL ERROR (`pbt`): unregistered type during SCC discovery")
                .element
        };
    }

    if bookkeeping
        .insert(
            vertex,
            VertexBookkeeping {
                global_visit_index: *global_visit_index,
                low_link: *global_visit_index,
                on_stack: true,
            },
        )
        .is_some()
    {
        panic!("INTERNAL ERROR (`pbt`): revisiting during SCC discovery")
    }
    *global_visit_index += 1;
    stack.push(vertex);

    #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
    for child in outgoing_edges(vertex) {
        if quotient.root(*child).is_some() {
            continue; // SCC already discovered from some other type
        }

        if let Some(child_books) = bookkeeping.get(child) {
            if child_books.on_stack {
                let child_index = child_books.global_visit_index;
                let v_books = get_mut!(&vertex);
                if child_index < v_books.low_link {
                    v_books.low_link = child_index;
                }
            }
        } else {
            let () = tarjan::<OutgoingEdges, Vertex>(
                *child,
                outgoing_edges,
                quotient,
                global_visit_index,
                bookkeeping,
                stack,
            );
            let child_low_link = get!(child).low_link;
            let v_books = get_mut!(&vertex);
            if child_low_link < v_books.low_link {
                v_books.low_link = child_low_link;
            }
        }
    }

    // Check if `vertex` is the root of an SCC,
    // i.e. the first visited within that SCC:
    let v_books = get!(&vertex);
    if v_books.global_visit_index == v_books.low_link {
        let n_before_stack = {
            // Mutually inductive types are small groups, so use linear search from the back:
            let mut i = stack.len() - 1;
            while *stack
                .get(i)
                .expect("INTERNAL ERROR (`pbt`): stack invariant violated during SCC discovery")
                != vertex
            {
                i -= 1;
            }
            i
        };

        for &popped in stack
            .get(n_before_stack..)
            .expect("INTERNAL ERROR (`pbt`): stack invariant violated during SCC discovery")
        {
            let immediately_reachable = outgoing_edges(popped)
                .iter()
                .copied()
                .filter(|dst| !bookkeeping.get(dst).is_some_and(|books| books.on_stack))
                .map(|dst| root!(dst))
                .collect();
            let mut elements = set();
            let _: bool = elements.insert(popped);
            quotient.insert_singleton(
                popped,
                Arc::new(QuotientVertex {
                    elements,
                    immediately_reachable,
                }),
            );
            let () = quotient.merge(vertex, popped, |lhs, rhs| {
                Arc::new(QuotientVertex {
                    elements: lhs.elements.union(&rhs.elements).copied().collect(),
                    immediately_reachable: lhs
                        .immediately_reachable
                        .union(&rhs.immediately_reachable)
                        .copied()
                        .collect(),
                })
            });
        }

        for &popped in stack
            .get(n_before_stack..)
            .expect("INTERNAL ERROR (`pbt`): stack invariant violated during SCC discovery")
        {
            get_mut!(&popped).on_stack = false;
        }

        let () = stack.truncate(n_before_stack);
    }
}

/// Compute the unavoidable vertices forced by a single variant.
#[inline]
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "For internal use only: invariant violations should fail loudly."
)]
fn variant_unavoidable<Vertex, Variant, FieldsOf>(
    variant: &Variant,
    cached: &HashMap<Vertex, Arc<HashSet<Vertex>>>,
    scc_acc: &HashMap<Vertex, HashSet<Vertex>>,
    fields_of: &FieldsOf,
) -> HashSet<Vertex>
where
    Vertex: Copy + Eq + Hash,
    FieldsOf: Fn(&Variant) -> &Multiset<Vertex>,
{
    let mut union = set();

    #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
    for field in fields_of(variant).counts.keys() {
        if let Some(local_unavoidables) = scc_acc.get(field) {
            let () = union.extend(local_unavoidables.iter().copied());
        } else {
            let cached_unavoidables = cached
                .get(field)
                .expect("INTERNAL ERROR (`pbt`): missing unavoidability entry");
            let () = union.extend(cached_unavoidables.iter().copied());
        }
    }
    union
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "Failing tests ought to panic.")]

    use {super::*, crate::hash::set, pretty_assertions::assert_eq};

    type AdtGraph = HashMap<u8, Vec<Multiset<u8>>>;
    type Graph = HashMap<u8, HashSet<u8>>;

    fn adt(variants: &[&[u8]]) -> Vec<Multiset<u8>> {
        variants
            .iter()
            .copied()
            .map(|fields| fields.iter().copied().collect())
            .collect()
    }

    fn adt_graph<const N: usize>(entries: [(u8, Vec<Multiset<u8>>); N]) -> AdtGraph {
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

    fn graph(edges: &[(u8, &[u8])]) -> Graph {
        let mut graph = map();
        for &(source, destinations) in edges {
            assert_eq!(
                graph.insert(source, vertex_set(destinations)),
                None,
                "test graph has a duplicate source vertex",
            );
        }
        graph
    }

    fn graph_from_adts(vertices: &AdtGraph) -> Graph {
        let mut graph = map();
        #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
        for (vertex, adt) in vertices {
            let mut destinations = set();
            for variant in adt {
                destinations.extend(variant.counts.keys().copied());
            }
            assert_eq!(
                graph.insert(*vertex, destinations),
                None,
                "test ADT graph has a duplicate vertex",
            );
        }
        graph
    }

    fn cached_unavoidables(cache: &HashMap<u8, Arc<HashSet<u8>>>, vertex: u8) -> HashSet<u8> {
        cache.get(&vertex).unwrap().as_ref().clone()
    }

    fn fields_of_multiset(variant: &Multiset<u8>) -> &Multiset<u8> {
        variant
    }

    fn outgoing_edges<'graph>(graph: &'graph Graph) -> impl Fn(u8) -> &'graph HashSet<u8> + 'graph {
        move |vertex| &graph[&vertex]
    }

    fn update_unavoidables_from(vertices: &AdtGraph, root: u8) -> HashMap<u8, Arc<HashSet<u8>>> {
        let graph = graph_from_adts(vertices);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();
        update_quotient_reachable_from(root, &edges, &mut quotient);

        let mut cache = map();
        update_unavoidable(
            root,
            &mut cache,
            vertices,
            &mut quotient,
            &variants_of_adt,
            &fields_of_multiset,
        );
        cache
    }

    fn variants_of_adt(adt: &Vec<Multiset<u8>>) -> &[Multiset<u8>] {
        adt.as_slice()
    }

    fn immediate_roots(
        quotient: &mut UnionFind<u8, Arc<QuotientVertex<u8>>>,
        vertex: u8,
    ) -> HashSet<RootElement<u8>> {
        quotient
            .root(vertex)
            .unwrap()
            .metadata
            .immediately_reachable
            .clone()
    }

    fn reachable_roots(
        quotient: &mut UnionFind<u8, Arc<QuotientVertex<u8>>>,
        vertex: u8,
    ) -> HashSet<RootElement<u8>> {
        let root = quotient.root(vertex).unwrap().element;
        let mut cache = map();
        reachable(&mut cache, quotient, root).as_ref().clone()
    }

    fn root_set(
        quotient: &mut UnionFind<u8, Arc<QuotientVertex<u8>>>,
        vertices: &[u8],
    ) -> HashSet<RootElement<u8>> {
        let mut roots = set();
        for vertex in vertices {
            let _: bool = roots.insert(quotient.root(*vertex).unwrap().element);
        }
        roots
    }

    #[test]
    fn singleton_without_edges_has_empty_quotient_edges() {
        let graph = graph(&[(1, &[])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_eq!(
            immediate_roots(&mut quotient, 1),
            root_set(&mut quotient, &[])
        );
    }

    #[test]
    fn self_loop_does_not_create_a_quotient_self_edge() {
        let graph = graph(&[(1, &[1])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_eq!(
            immediate_roots(&mut quotient, 1),
            root_set(&mut quotient, &[])
        );
    }

    #[test]
    fn two_vertex_cycle_becomes_one_quotient_vertex_without_self_edges() {
        let graph = graph(&[(1, &[2]), (2, &[1])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_eq!(
            quotient.root(1).unwrap().element,
            quotient.root(2).unwrap().element
        );
        assert_eq!(
            immediate_roots(&mut quotient, 1),
            root_set(&mut quotient, &[])
        );
    }

    #[test]
    fn mutually_inductive_component_keeps_only_external_quotient_edges() {
        let graph = graph(&[(1, &[2, 3]), (2, &[1, 3]), (3, &[])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_eq!(
            quotient.root(1).unwrap().element,
            quotient.root(2).unwrap().element
        );
        assert_ne!(
            quotient.root(1).unwrap().element,
            quotient.root(3).unwrap().element
        );
        assert_eq!(
            immediate_roots(&mut quotient, 1),
            root_set(&mut quotient, &[3])
        );
        assert_eq!(
            immediate_roots(&mut quotient, 3),
            root_set(&mut quotient, &[])
        );
    }

    #[test]
    fn later_rooted_run_reuses_an_already_quotiented_subgraph() {
        let graph = graph(&[(1, &[2, 4]), (2, &[3]), (3, &[2, 4]), (4, &[])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(2, &edges, &mut quotient);
        let recursive_child_root = quotient.root(2).unwrap().element;
        let leaf_root = quotient.root(4).unwrap().element;

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_ne!(quotient.root(1).unwrap().element, recursive_child_root);
        assert_eq!(quotient.root(2).unwrap().element, recursive_child_root);
        assert_eq!(quotient.root(3).unwrap().element, recursive_child_root);
        assert_eq!(quotient.root(4).unwrap().element, leaf_root);
        assert_eq!(
            immediate_roots(&mut quotient, 1),
            root_set(&mut quotient, &[2, 4])
        );
        assert_eq!(
            immediate_roots(&mut quotient, 2),
            root_set(&mut quotient, &[4])
        );
    }

    #[test]
    fn quotient_reachability_is_reflexive_for_leaf_sccs() {
        let graph = graph(&[(1, &[])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_eq!(
            reachable_roots(&mut quotient, 1),
            root_set(&mut quotient, &[1])
        );
    }

    #[test]
    fn quotient_reachability_follows_diamond_to_common_leaf() {
        let graph = graph(&[(1, &[2, 3]), (2, &[4]), (3, &[4]), (4, &[])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_eq!(
            reachable_roots(&mut quotient, 1),
            root_set(&mut quotient, &[1, 2, 3, 4])
        );
        assert_eq!(
            reachable_roots(&mut quotient, 2),
            root_set(&mut quotient, &[2, 4])
        );
    }

    #[test]
    fn quotient_reachability_uses_collapsed_scc_roots() {
        let graph = graph(&[(1, &[2]), (2, &[1, 3]), (3, &[])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update_quotient_reachable_from(1, &edges, &mut quotient);

        assert_eq!(
            quotient.root(1).unwrap().element,
            quotient.root(2).unwrap().element
        );
        assert_eq!(
            reachable_roots(&mut quotient, 1),
            root_set(&mut quotient, &[1, 3])
        );
    }

    #[test]
    fn unavoidability_diamond_keeps_only_common_branch_dependencies() {
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
    fn unavoidability_leaf_variant_escapes_recursive_field() {
        // 1 = leaf | 2; 2 = leaf.
        let vertices = adt_graph([(1, adt(&[&[], &[2]])), (2, adt(&[&[]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[2]));
    }

    #[test]
    fn unavoidability_mutual_recursion_with_escape_is_asymmetric() {
        // 1 = 2; 2 = leaf | 1.
        let vertices = adt_graph([(1, adt(&[&[2]])), (2, adt(&[&[], &[1]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 2]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[2]));
    }

    #[test]
    fn unavoidability_mutual_recursion_without_escape_is_symmetric() {
        // 1 = 2; 2 = 1.
        let vertices = adt_graph([(1, adt(&[&[2]])), (2, adt(&[&[1]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 2]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[1, 2]));
    }

    #[test]
    fn unavoidability_forced_external_dependency_propagates_around_cycle() {
        // 1 = (2, 3); 2 = 1; 3 = leaf.
        let vertices = adt_graph([(1, adt(&[&[2, 3]])), (2, adt(&[&[1]])), (3, adt(&[&[]]))]);

        let cache = update_unavoidables_from(&vertices, 1);

        assert_eq!(cached_unavoidables(&cache, 1), vertex_set(&[1, 2, 3]));
        assert_eq!(cached_unavoidables(&cache, 2), vertex_set(&[1, 2, 3]));
        assert_eq!(cached_unavoidables(&cache, 3), vertex_set(&[3]));
    }

    fn vertex_set(vertices: &[u8]) -> HashSet<u8> {
        let mut set = set();
        for vertex in vertices {
            let _: bool = set.insert(*vertex);
        }
        set
    }
}
