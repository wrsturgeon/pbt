//! Tarjan's strongly connected components algorithm.

use {
    crate::{
        hash::map,
        union_find::{RootElement, UnionFind},
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::hash::Hash,
};

/// All fields of all types within an SCC (i.e. a mutually inductive set of types)
/// that do not themselves belong to the SCC.
///
/// Recall that fields of each individual type are directed edges,
/// so directed edges out of an SCC are not a very well-defined concept,
/// but they could be seen as representing "optional dependencies,"
/// i.e. that there exists a generator path that contains a term of this type.
#[non_exhaustive]
pub struct QuotientVertex<Vertex> {
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
        {
            let immediately_reachable = outgoing_edges(vertex)
                .iter()
                .copied()
                .map(|dst| root!(dst))
                .collect();
            quotient.insert_singleton(
                // SAFETY: Unions favor the left argument, so this will be the root.
                vertex,
                Arc::new(QuotientVertex {
                    immediately_reachable,
                }),
            );
        }

        'pop: loop {
            let popped = stack
                .pop()
                .expect("INTERNAL ERROR (`pbt`): violated stack invariant during SCC discovery");
            get_mut!(&popped).on_stack = false;
            if popped == vertex {
                break 'pop;
            }

            let immediately_reachable = outgoing_edges(popped)
                .iter()
                .copied()
                .map(|dst| root!(dst))
                .collect();
            quotient.insert_singleton(
                // SAFETY: Unions favor the left argument, so this will be the root.
                popped,
                Arc::new(QuotientVertex {
                    immediately_reachable,
                }),
            );
            let () = quotient.merge(vertex, popped, |lhs, rhs| {
                Arc::new(QuotientVertex {
                    immediately_reachable: lhs
                        .immediately_reachable
                        .union(&rhs.immediately_reachable)
                        .copied()
                        .collect(),
                })
            });
        }
    }
}
