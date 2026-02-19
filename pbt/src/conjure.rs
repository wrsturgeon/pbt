use {
    crate::count::{Cardinality, Count},
    alloc::collections::BinaryHeap,
    core::{array, num::NonZero},
    std::iter,
    wyrand::WyRand,
};

pub trait Conjure: 'static + Count + Sized {
    /// Deterministically generate an arbitrary term of type `Self`.
    /// # Invariant
    /// This function must return `None` if and only if
    /// this type is uninstantiable with finite memory
    /// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
    #[must_use]
    fn conjure(seed: Seed) -> Result<Self, Uninstantiable>;

    /// Iterate over the full set of corner cases of this type.
    #[must_use]
    fn corners() -> Box<dyn Iterator<Item = Self>>;

    /// The exhaustive set of ways to create a value of this type,
    /// each annotated with the cardinality of that variant.
    #[must_use]
    #[expect(clippy::type_complexity, reason = "not very complex")]
    fn variants() -> impl Iterator<Item = (Cardinality, fn(Seed) -> Self)>;

    /// Deterministically generate an arbitrary term that does not contain `Self`:
    /// for example, a binary tree could create only a leaf (hence the name)
    /// because a node would require two children (i.e. subtrees) of type `Self`,
    /// and a linked-list could create only a sentinel/empty node.
    /// This notion turns out to be straightforwardly generalizable to all types
    /// via the `Count` trait: specifically, choose uniformly among
    /// variants with `Cardinality::Finite`.
    /// # Invariant
    /// This function must return `None` if and only if
    /// this type is uninstantiable with finite memory
    /// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
    #[must_use]
    fn leaf(seed: Seed) -> Result<Self, Uninstantiable>;
}

#[expect(clippy::module_name_repetitions, reason = "for unqualified use")]
pub trait ConjureAsync: Conjure + Send + Sync {
    /// Asynchronously and deterministically generate an arbitrary term of type `Self`.
    /// (Asynchronous generation is potentially useful if generation could be
    /// massively parallelized over independent subtrees, for example.)
    /// # Invariant
    /// This function must return `None` if and only if
    /// this type is uninstantiable with finite memory
    /// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
    #[must_use]
    fn conjure_async(
        seed: Seed,
    ) -> impl Future<Output = Result<Self, Uninstantiable>> + Send + Sync;
}

/// Deterministic pseudorandom number generator.
/// The `size` field controls the
/// maximum size of inductive data structures:
/// any type with finitely many "elements" (e.g. `usize`) counts as a leaf,
/// whereas any potentially infinite type (e.g. `Vec<()>`)
/// counts as an internal node and contributes to a term's "size."
#[derive(/* NOT Clone, NOT Copy, */ Debug)]
pub struct Seed {
    seed: u64,
    size: usize,
}

/// An infinite iterator of increasingly large seeds.
pub struct Seeds(Seed);

/// Error: uninstantiable with finite memory
/// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Uninstantiable;

// TODO: try `smallvec`
/// The exhaustive set of ways to create a value of this type,
/// partitioned into those of finite or infinite cardinality
/// (i.e. leaves and internal nodes, respectively).
#[derive(Clone, Debug)]
pub struct Variants<T: Conjure> {
    pub internal_nodes: Vec<fn(Seed) -> T>,
    pub leaves: Vec<fn(Seed) -> T>,
}

impl Seed {
    #[inline]
    #[must_use]
    pub const fn new(size: usize) -> Self {
        Self {
            seed: 1337_1337_1337_1337_1337,
            size,
        }
    }

    /// Use a stars-and-bars-style subroutine to
    /// split a total size among a known number of children
    /// and generate pseudorandom seeds for each.
    #[inline]
    #[must_use]
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "not critical"
    )]
    pub fn split<const N: usize>(mut self) -> [Self; N] {
        if let Some(nz) = NonZero::new(self.size) {
            // Make a heap containing the endpoints (0 and `size`) as well as
            // `N - 1` random indices between `0` and `size`, inclusize.
            // This means there are `N + 1` indices total, so we can sample
            // `N` ranges from one to the next if we iterate in sorted order,
            // and those ranges will cover the full `0..=size`.
            let mut heap =
                BinaryHeap::from_iter([0, self.size].into_iter().chain(const { 1..N }.map(|_| {
                    let unrestricted = self.prng();
                    let unrestricted = unrestricted as usize;
                    unrestricted % nz
                })));
            array::from_fn(|_| {
                // SAFETY: Size of `heap` is exactly `N + 1`.
                let lhs = unsafe { heap.pop().unwrap_unchecked() };
                // SAFETY: Size of `heap` is exactly `N + 1`.
                let rhs = *unsafe { heap.peek().unwrap_unchecked() };
                // SAFETY: `heap` is a max-heap, and `rhs` came after `lhs`.
                let difference = unsafe { lhs.unchecked_sub(rhs) };
                Self {
                    seed: self.prng(),
                    size: difference,
                }
            })
        } else {
            array::from_fn(|_| Self {
                seed: self.prng(),
                size: 0,
            })
        }
    }

    #[inline]
    pub fn stream(mut self) -> impl Iterator<Item = Self> {
        core::iter::repeat_with(move || {
            let size = if let Some(nz) = NonZero::new(self.size) {
                self.prng() as usize % nz
            } else {
                0
            };
            self.size -= size;
            Seed {
                seed: self.prng(),
                size,
            }
        })
    }

    /// Generate a pseudorandom `u64`,
    /// ignoring this seed's `size` field.
    #[inline]
    #[must_use]
    pub const fn prng(&mut self) -> u64 {
        let (prng, seed) = WyRand::gen_u64(self.seed);
        self.seed = seed;
        prng
    }

    #[inline]
    #[must_use]
    pub const fn prng_bool(&mut self) -> bool {
        (self.prng() & 1) != 0
    }

    /// With a chance inversely proportional to `size`,
    /// stop recursing right now.
    #[inline]
    #[must_use]
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "not critical"
    )]
    pub fn should_recurse(&mut self) -> bool {
        // With a chance inversely proportional to `size`, stop here
        if let Some(nz) = self.size.checked_add(1)
            // SAFETY: Added 1 above, and overflow already exited.
            && (self.prng() as usize % unsafe { NonZero::new_unchecked(nz) }) == 0
        {
            false
        } else {
            /*
            // Decrease the remaining size, since
            // an extra node now exists (this one).
            self.size = self.size.checked_sub(1)?;
            */

            true
        }
    }
}

impl Count for Seed {
    const CARDINALITY: Cardinality = Cardinality::Finite;
}

impl Conjure for Seed {
    #[inline]
    fn conjure(seed: Seed) -> Result<Self, Uninstantiable> {
        Ok(seed) // important later on that this is exact
    }

    #[inline]
    fn corners() -> Box<dyn Iterator<Item = Self>> {
        Box::new(Conjure::corners().map(|(seed, size)| Self { seed, size }))
    }

    #[inline]
    fn leaf(seed: Seed) -> Result<Self, Uninstantiable> {
        Conjure::leaf(seed).map(|(seed, size)| Self { seed, size })
    }

    #[inline]
    fn variants() -> impl Iterator<Item = (Cardinality, fn(Seed) -> Self)> {
        iter::once((
            Self::CARDINALITY,
            (|seed| unsafe { Self::conjure(seed).unwrap_unchecked() }) as fn(_) -> _,
        ))
    }
}

impl Iterator for Seeds {
    type Item = Seed;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = Seed {
            seed: self.0.prng(),
            size: self.0.size,
        };
        self.0.size = self.0.size.checked_add(1)?;
        Some(next)
    }
}

/// The exhaustive set of ways to create a value of this type,
/// partitioned into those of finite or infinite cardinality
/// (i.e. leaves and internal nodes, respectively).
#[inline]
pub fn internal_nodes<T: Conjure>() -> impl Iterator<Item = fn(Seed) -> T> {
    T::variants()
        .filter_map(|(cardinality, f)| matches!(cardinality, Cardinality::Infinite).then_some(f))
}

/// The exhaustive set of ways to create a value of this type,
/// partitioned into those of finite or infinite cardinality
/// (i.e. leaves and internal nodes, respectively).
#[inline]
pub fn leaves<T: Conjure>() -> impl Iterator<Item = fn(Seed) -> T> {
    T::variants()
        .filter_map(|(cardinality, f)| matches!(cardinality, Cardinality::Finite).then_some(f))
}

/// An infinite iterator of increasingly large seeds.
#[inline]
pub fn seeds() -> impl Iterator<Item = Seed> {
    Seeds(Seed::new(0))
}

/// The exhaustive set of ways to create a value of this type,
/// partitioned into those of finite or infinite cardinality
/// (i.e. leaves and internal nodes, respectively).
#[inline]
pub fn variants<T: Conjure>() -> Variants<T> {
    let mut internal_nodes = vec![];
    let mut leaves = vec![];
    for (cardinality, f) in T::variants() {
        match cardinality {
            Cardinality::Empty => {}
            Cardinality::Finite => leaves.push(f),
            Cardinality::Infinite => internal_nodes.push(f),
        }
    }
    Variants {
        internal_nodes,
        leaves,
    }
}
