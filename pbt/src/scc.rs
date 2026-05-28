//! Tarjan's strongly connected components algorithm.

use {
    crate::{hash::map, union_find::UnionFind},
    ahash::HashMap,
    core::hash::Hash,
};

/// Per-vertex bookkeeping for Tarjan's SCC algorithm.
#[non_exhaustive]
struct VertexBookkeeping {
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
pub(crate) fn update<Destinations, OutgoingEdges, Vertex>(
    vertex: Vertex,
    outgoing_edges: &OutgoingEdges,
    quotient: &mut UnionFind<Vertex>,
) where
    Destinations: Iterator<Item = Vertex>,
    OutgoingEdges: Fn(Vertex) -> Destinations,
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
    reason = "Internal invariants: violations should fail loudly."
)]
fn tarjan<Destinations, OutgoingEdges, Vertex>(
    vertex: Vertex,
    outgoing_edges: &OutgoingEdges,
    quotient: &mut UnionFind<Vertex>,
    global_visit_index: &mut usize,
    bookkeeping: &mut HashMap<Vertex, VertexBookkeeping>,
    stack: &mut Vec<Vertex>,
) where
    Destinations: Iterator<Item = Vertex>,
    OutgoingEdges: Fn(Vertex) -> Destinations,
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

    for child in outgoing_edges(vertex) {
        if quotient.root(child).is_some() {
            continue; // SCC already discovered from some other type
        }

        if let Some(child_books) = bookkeeping.get(&child) {
            if child_books.on_stack {
                let child_index = child_books.global_visit_index;
                let v_books = get_mut!(&vertex);
                if child_index < v_books.low_link {
                    v_books.low_link = child_index;
                }
            }
        } else {
            let () = tarjan::<Destinations, OutgoingEdges, Vertex>(
                child,
                outgoing_edges,
                quotient,
                global_visit_index,
                bookkeeping,
                stack,
            );
            let child_low_link = get!(&child).low_link;
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
            let () = quotient.insert_singleton(popped);
            let () = quotient.merge(vertex, popped);
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

    use {
        super::*, crate::hash::set, ahash::HashSet, alloc::sync::Arc, core::iter,
        pretty_assertions::assert_eq, std::collections::hash_set,
    };

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

    fn outgoing_edges<'g>(graph: &'g Graph) -> impl Fn(u8) -> iter::Copied<hash_set::Iter<'g, u8>> {
        move |vertex| graph[&vertex].iter().copied()
    }

    #[test]
    fn singleton_without_edges_gets_singleton_root() {
        let graph = graph(&[(1, &[])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update(1, &edges, &mut quotient);

        assert_eq!(quotient.root(1).unwrap().cardinality.get(), 1);
    }

    #[test]
    fn self_loop_gets_singleton_root() {
        let graph = graph(&[(1, &[1])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update(1, &edges, &mut quotient);

        assert_eq!(quotient.root(1).unwrap().cardinality.get(), 1);
    }

    #[test]
    fn two_vertex_cycle_becomes_one_quotient_vertex() {
        let graph = graph(&[(1, &[2]), (2, &[1])]);
        let edges = outgoing_edges(&graph);
        let mut quotient = UnionFind::new();

        update(1, &edges, &mut quotient);

        assert_eq!(
            quotient.root(1).unwrap().element,
            quotient.root(2).unwrap().element
        );
        assert_eq!(quotient.root(1).unwrap().cardinality.get(), 2);
    }

    #[test]
    fn mutually_inductive_component_merges_only_cycle() {
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
        assert_eq!(quotient.root(1).unwrap().cardinality.get(), 2);
        assert_eq!(quotient.root(3).unwrap().cardinality.get(), 1);
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
        assert_eq!(quotient.root(1).unwrap().cardinality.get(), 1);
        assert_eq!(quotient.root(2).unwrap().cardinality.get(), 2);
        assert_eq!(quotient.root(4).unwrap().cardinality.get(), 1);
    }

    fn vertex_set(vertices: &[u8]) -> HashSet<u8> {
        let mut set = set();
        for vertex in vertices {
            let _: bool = set.insert(*vertex);
        }
        set
    }
}
