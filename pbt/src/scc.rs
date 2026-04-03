//! Disjoint sets of mutually inductive types,
//! represented using the standard Union-Find algorithm.

use {
    crate::{SEED, reflection::Type},
    ahash::{HashMap, RandomState},
    core::{
        mem,
        num::NonZero,
        ops::{Deref, DerefMut},
    },
    std::collections::{BTreeMap, BTreeSet},
};

/// Metadata stored in the root of a set.
#[derive(Clone, Debug)]
pub struct Metadata {
    /// The cardinality of the set.
    /// Note that this is `None` for non-inductive types
    /// and `Some(1)` for types with only a self-loop.
    pub cardinality: Option<NonZero<usize>>,
    /// Immediately reachable types.
    pub edges: BTreeSet<Type>,
    /// Type ID of the root.
    pub ty: Type,
}

/// Disjoint sets of mutually inductive types,
/// represented using the standard Union-Find algorithm.
#[derive(Debug)]
pub enum Node {
    /// A non-root set element.
    Parent(Type),
    /// The root of a set.
    Root(Metadata),
}

/// Disjoint sets of mutually inductive types,
/// represented using the standard Union-Find algorithm.
#[derive(Debug)]
pub struct StronglyConnectedComponents {
    /// A map from each type to a union-find node.
    /// Note that this is `RwLock`ed to prevent
    /// race conditions when comparing two nodes,
    /// since that operation requires looking up
    /// each node's root separately (usually sequentially).
    nodes: HashMap<Type, Node>,
}

/// Vertex-level metadata for Tarjan's strongly connected components algorithm.
/// # Source
/// <https://en.wikipedia.org/wiki/Tarjan's_strongly_connected_components_algorithm#The_algorithm_in_pseudocode>.
#[derive(Debug, Eq, PartialEq)]
pub struct TarjanMetadata {
    /// Global index in visit order.
    index: usize,
    /// Clever quantity used to measure SCC-ness.
    lowlink: usize,
}

impl StronglyConnectedComponents {
    /// Merge two disjoint sets to form their union.
    #[inline]
    pub fn merge(&mut self, lhs: Type, rhs: Type) {
        merge(&mut self.nodes, lhs, rhs)
    }

    /// Empty set of disjoint sets.
    #[inline]
    pub fn new() -> Self {
        Self {
            nodes: HashMap::with_hasher(RandomState::with_seed(usize::from(SEED))),
        }
    }

    /// Find the (arbitrary) root node of a set
    /// from any of its elements
    /// *and* run path shortening while doing so.
    #[inline]
    pub fn root(&mut self, element: Type) -> Option<Type> {
        root(&mut self.nodes, element)
    }

    /// Tarjan's strongly connected components algorithm.
    /// # Source
    /// <https://en.wikipedia.org/wiki/Tarjan's_strongly_connected_components_algorithm#The_algorithm_in_pseudocode>.
    #[inline]
    pub fn tarjan(&mut self) {
        let mut index = 0;
        let mut metadata = BTreeMap::new();
        let mut stack = vec![];
        let vertices: Vec<Type> = self.nodes.keys().copied().collect();
        for vertex in vertices {
            let () = self.tarjan_dfs(vertex, &mut metadata, &mut stack, &mut index);
        }
    }

    /// Tarjan's strongly connected components algorithm.
    /// # Source
    /// <https://en.wikipedia.org/wiki/Tarjan's_strongly_connected_components_algorithm#The_algorithm_in_pseudocode>.
    #[inline]
    #[expect(clippy::expect_used, clippy::panic, reason = "internal invariants")]
    pub fn tarjan_dfs(
        &mut self,
        vertex: Type,
        metadata: &mut BTreeMap<Type, TarjanMetadata>,
        stack: &mut Vec<Type>,
        index: &mut usize,
    ) {
        // If not, proceed:
        // (Note that TOCTOU is a non-issue since `metadata` is `&mut`.)
        let overwritten: Option<_> = metadata.insert(
            vertex,
            TarjanMetadata {
                index: *index,
                lowlink: *index,
            },
        );
        debug_assert_eq!(overwritten, None, "internal `pbt` error: TOCTOU");
        {
            #![expect(clippy::arithmetic_side_effects, reason = "constrained by hardware")]
            *index += 1;
        }
        stack.push(vertex);

        let Some(node) = self.nodes.get(&vertex) else {
            panic!("internal `pbt` error: unregistered SCC element `{vertex:#?}`")
        };
        let Node::Root(Metadata { ref edges, .. }) = *node else {
            return;
        };
        let edges: Vec<Type> = edges.iter().copied().collect();
        for successor in edges {
            if let Some(&TarjanMetadata {
                index: successor_index,
                ..
            }) = metadata.get(&successor)
            {
                // Check if that "successor" is on the stack
                // (i.e. it's really the last chain in a loop):
                if stack.iter().rev().any(|&t| t == vertex) {
                    let this_lowlink = &mut metadata
                        .get_mut(&vertex)
                        .expect("internal `pbt` error: schrodinger's metadata")
                        .lowlink;
                    let new_lowlink = (*this_lowlink).min(successor_index);
                    *this_lowlink = new_lowlink;
                }
            } else {
                let () = self.tarjan_dfs(successor, metadata, stack, index);
                let successor_lowlink = metadata
                    .get(&successor)
                    .expect("internal `pbt` error: schrodinger's metadata")
                    .lowlink;
                let this_lowlink = &mut metadata
                    .get_mut(&vertex)
                    .expect("internal `pbt` error: schrodinger's metadata")
                    .lowlink;
                let new_lowlink = (*this_lowlink).min(successor_lowlink);
                *this_lowlink = new_lowlink;
            }
        }

        // Check if this is the root of an SCC:
        let metadata = metadata
            .get(&vertex)
            .expect("internal `pbt` error: schrodinger's metadata");
        if metadata.index == metadata.lowlink {
            'pop: loop {
                let popped = stack
                    .pop()
                    .expect("internal `pbt` error: schrodinger's stack");
                let () = self.merge(vertex, popped);
                if popped == vertex {
                    break 'pop;
                }
            }
        }
    }
}

impl Deref for StronglyConnectedComponents {
    type Target = HashMap<Type, Node>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl DerefMut for StronglyConnectedComponents {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.nodes
    }
}

/// Merge two disjoint sets to form their union.
#[inline]
#[expect(clippy::expect_used, clippy::panic, reason = "internal invariants")]
fn merge(nodes: &mut HashMap<Type, Node>, lhs: Type, rhs: Type) {
    let mut lhs = root(nodes, lhs).expect("internal `pbt` error: merging unregistered SCC element");
    let mut rhs = root(nodes, rhs).expect("internal `pbt` error: merging unregistered SCC element");

    if lhs == rhs {
        return;
    }

    let Some(&Node::Root(ref lhs_meta)) = nodes.get(&lhs) else {
        panic!("internal `pbt` error: `scc::root` is not idempotent")
    };
    let Some(&Node::Root(ref rhs_meta)) = nodes.get(&rhs) else {
        panic!("internal `pbt` error: `scc::root` is not idempotent")
    };
    if rhs_meta.cardinality > lhs_meta.cardinality {
        let () = mem::swap(&mut lhs, &mut rhs);
    }

    let edges: Vec<_> = lhs_meta
        .edges
        .iter()
        .chain(&rhs_meta.edges)
        .copied()
        .collect();
    let meta = Metadata {
        #[expect(clippy::arithmetic_side_effects, reason = "constrained by hardware")]
        cardinality: NonZero::new(
            lhs_meta.cardinality.map_or(1, NonZero::get) + // disjoint (invariant)
            rhs_meta.cardinality.map_or(1, NonZero::get),
        ),
        edges: edges
            .into_iter()
            .map(|edge| {
                root(nodes, edge).expect("internal `pbt` error: invalid (transitive) parent in SCC")
            })
            .filter(|&root| root != lhs)
            .collect(),
        ty: lhs,
    };

    let _: Option<Node> = nodes.insert(rhs, Node::Parent(lhs));
    let _: Option<Node> = nodes.insert(lhs, Node::Root(meta));
}

/// Find the (arbitrary) root node of a set
/// from any of its elements
/// *and* run path shortening while doing so.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::unwrap_in_result,
    reason = "internal invariants"
)]
fn root(nodes: &mut HashMap<Type, Node>, element: Type) -> Option<Type> {
    let node = nodes.get(&element)?;
    let parent = match *node {
        Node::Root(ref metadata) => return Some(metadata.ty),
        Node::Parent(parent) => parent,
    };
    let root =
        root(nodes, parent).expect("internal `pbt` error: invalid (transitive) parent in SCC");
    let () = debug_assert_ne!(root, element, "internal `pbt` error: union-find cycle");
    let _: Option<Node> = nodes.insert(element, Node::Parent(root));
    Some(root)
}
