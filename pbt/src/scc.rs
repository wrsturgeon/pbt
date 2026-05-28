//! Tarjan's strongly connected components algorithm.

use {
    crate::{
        hash::{map, set},
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
    pub outgoing_edges: HashSet<RootElement<Vertex>>,
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
pub fn update<OutgoingEdges, Vertex>(
    vertex: Vertex,
    outgoing_edges: &OutgoingEdges,
    quotient: &mut UnionFind<Vertex, Arc<QuotientVertex<Vertex>>>,
) where
    OutgoingEdges: Fn(Vertex) -> Arc<HashSet<Vertex>>,
    Vertex: Copy + Eq + Hash,
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
fn tarjan<OutgoingEdges, Vertex>(
    vertex: Vertex,
    outgoing_edges: &OutgoingEdges,
    quotient: &mut UnionFind<Vertex, Arc<QuotientVertex<Vertex>>>,
    global_visit_index: &mut usize,
    bookkeeping: &mut HashMap<Vertex, VertexBookkeeping>,
    stack: &mut Vec<Vertex>,
) where
    OutgoingEdges: Fn(Vertex) -> Arc<HashSet<Vertex>>,
    Vertex: Copy + Eq + Hash,
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
    for child in &*outgoing_edges(vertex) {
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
            let outgoing_edges_of_popped = outgoing_edges(popped)
                .iter()
                .copied()
                .filter(|dst| !bookkeeping.get(dst).is_some_and(|books| books.on_stack))
                .map(|dst| root!(dst))
                .collect();
            let mut elements = set();
            let _: bool = elements.insert(popped);
            let () = quotient.insert_singleton(
                popped,
                Arc::new(QuotientVertex {
                    elements,
                    outgoing_edges: outgoing_edges_of_popped,
                }),
            );
            let () = quotient.merge(vertex, popped, |lhs, rhs| {
                Arc::new(QuotientVertex {
                    elements: lhs.elements.union(&rhs.elements).copied().collect(),
                    outgoing_edges: lhs
                        .outgoing_edges
                        .union(&rhs.outgoing_edges)
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

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "Failing tests ought to panic.")]

    use {super::*, crate::hash::set, pretty_assertions::assert_eq};

    type Graph = HashMap<u8, Arc<HashSet<u8>>>;

    fn graph(edges: &[(u8, &[u8])]) -> Graph {
        let mut graph = map();
        for &(source, destinations) in edges {
            assert_eq!(
                graph.insert(source, Arc::new(vertex_set(destinations))),
                None,
                "test graph has a duplicate source vertex",
            );
        }
        graph
    }

    fn outgoing_edges(graph: &Graph) -> impl Fn(u8) -> Arc<HashSet<u8>> {
        move |vertex| Arc::clone(&graph[&vertex])
    }

    fn immediate_roots(
        quotient: &mut UnionFind<u8, Arc<QuotientVertex<u8>>>,
        vertex: u8,
    ) -> HashSet<RootElement<u8>> {
        quotient
            .root(vertex)
            .unwrap()
            .metadata
            .outgoing_edges
            .clone()
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

        update(1, &edges, &mut quotient);

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

        update(1, &edges, &mut quotient);

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

        update(1, &edges, &mut quotient);

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

        update(1, &edges, &mut quotient);

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

        update(2, &edges, &mut quotient);
        let recursive_child_root = quotient.root(2).unwrap().element;
        let leaf_root = quotient.root(4).unwrap().element;

        update(1, &edges, &mut quotient);

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

    fn vertex_set(vertices: &[u8]) -> HashSet<u8> {
        let mut set = set();
        for vertex in vertices {
            let _: bool = set.insert(*vertex);
        }
        set
    }
}
