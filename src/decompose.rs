use {
    crate::{
        conjure::{Conjure, Seed},
        count::{Cardinality, Count},
    },
    core::{fmt, iter},
};

/// This is a binary tree with *nothing* at each node,
/// represented as a vector (which you can think of as the left "spine" of the tree)
/// of this self-same data structure (which are children/subtrees branching off to the right).
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[expect(clippy::exhaustive_structs, reason = "intentional")]
pub struct Decomposition(pub Vec<Self>);

pub trait Decompose: Sized {
    /// Decompose this value into a
    /// binary tree with no data at each node.
    /// # Invariant
    /// This function must "round-trip" with `from_decomposition`: that is,
    /// `Self::from_decomposition(self.decompose()) == self`, but
    /// `self.decompose(Self::from_decomposition(d) == d` need not hold,
    /// e.g. if `d` has some extra complications or redundancies that cancel out.
    #[must_use]
    fn decompose(&self) -> Decomposition;

    /// Create a term of this type from a
    /// binary tree with no data at each node.
    /// If the binary tree has extraneous data, simply ignore it:
    /// use only the minimal set of data such that
    /// every term of this type can be created from (at least) one tree.
    /// # Invariant
    /// This function must return `None` if and only if
    /// this type is uninstantiable with finite memory
    /// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
    #[must_use]
    fn from_decomposition(d: &Decomposition) -> Option<Self>;
}

impl fmt::Debug for Decomposition {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Count for Decomposition {
    const CARDINALITY: Cardinality = Cardinality::Infinite;
}

impl Conjure for Decomposition {
    #[inline]
    fn conjure(seed: Seed, size: usize) -> Option<Self> {
        Vec::conjure(seed, size).map(Self)
    }

    #[inline]
    fn corners() -> impl Iterator<Item = Self> {
        // Can't use `Vec::corners().map(Self)`, since that would recurse infinitely.
        iter::once(Self(vec![]))
    }

    #[inline]
    fn leaf(seed: Seed) -> Option<Self> {
        Vec::leaf(seed).map(Self)
    }
}

/*
impl ConjureAsync for Decomposition {
    #[inline]
    async fn conjure_async(seed: Seed, size: usize) -> Option<Self> {
        Box::pin(Vec::conjure_async(seed, size)).await.map(Self)
    }
}
*/

impl Decompose for Decomposition {
    #[inline]
    fn decompose(&self) -> Decomposition {
        self.clone()
    }

    #[inline]
    fn from_decomposition(d: &Decomposition) -> Option<Self> {
        Some(d.clone())
    }
}

/// Check that `T::from_decomposition(t.decompose()) == t`.
/// # Panics
/// If that's not the case.
#[inline]
#[expect(clippy::panic, reason = "failing tests ought to panic")]
fn check_roundtrip_once<T: Conjure + fmt::Debug + Decompose + Eq>(t: &T) {
    let decomposed = t.decompose();
    let Some(recomposed) = T::from_decomposition(&decomposed) else {
        panic!("{t:?} --> {decomposed:?} --> None =/= Some(..)");
    };
    pretty_assertions::assert_eq!(
        recomposed,
        *t,
        "{t:?} --> {decomposed:?} --> {recomposed:?} =/= {t:?}"
    );
}

/// Check that `T::from_decomposition(t.decompose()) == t`
/// for a wide range of possible values of `t`.
/// # Panics
/// If a counterexample showed up.
#[inline]
#[expect(clippy::unwrap_used, reason = "failing tests ought to panic")]
pub fn check_roundtrip<T: Conjure + fmt::Debug + Decompose + Eq>() {
    const N_TRIALS: usize = 100;

    let mut seed = Seed::new();
    for corner in T::corners() {
        let () = check_roundtrip_once(&corner);
    }
    for size in 0..N_TRIALS {
        let t = T::conjure(seed.split(), size).unwrap();
        let () = check_roundtrip_once(&t);
    }
}
