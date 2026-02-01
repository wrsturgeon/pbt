use {
    crate::count::Count,
    alloc::collections::BinaryHeap,
    core::{array, iter, num::NonZero},
    wyrand::WyRand,
};

pub trait Conjure: Count + Sized {
    /// Deterministically generate an arbitrary term of type `Self`.
    /// The `size` parameter controls the
    /// maximum size of inductive data structures:
    /// any type with finitely many "elements" (e.g. `usize`) counts as a leaf,
    /// whereas any potentially infinite type (e.g. `Vec<()>`)
    /// counts as an internal node and contributes to a term's "size."
    /// # Invariant
    /// This function must return `None` if and only if
    /// this type is uninstantiable with finite memory
    /// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
    #[must_use]
    fn conjure(seed: Seed, size: usize) -> Option<Self>;
    /// Iterate over the full set of corner cases of this type.
    #[must_use]
    fn corners() -> impl Iterator<Item = Self>;
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
    fn leaf(seed: Seed) -> Option<Self>;
}

#[expect(clippy::module_name_repetitions, reason = "for unqualified use")]
pub trait ConjureAsync: Conjure + Send + Sync {
    /// Asynchronously and deterministically generate an arbitrary term of type `Self`.
    /// (Asynchronous generation is potentially useful if generation could be
    /// massively parallelized over independent subtrees, for example.)
    /// The `size` parameter controls the
    /// maximum size of inductive data structures:
    /// any type with finitely many "elements" (e.g. `usize`) counts as a leaf,
    /// whereas any potentially infinite type (e.g. `Vec<()>`)
    /// counts as an internal node and contributes to a term's "size."
    /// # Invariant
    /// This function must return `None` if and only if
    /// this type is uninstantiable with finite memory
    /// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
    #[must_use]
    fn conjure_async(seed: Seed, size: usize) -> impl Future<Output = Option<Self>> + Send + Sync;
}

#[derive(/* NOT Clone, NOT Copy, */ Debug)]
pub struct Seed(u64);

impl Seed {
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self(42) // yes, this is a bad initial state, but that's fine
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
    pub fn partition<const N: usize>(&mut self, size: usize) -> [(Self, usize); N] {
        if let Some(nz) = NonZero::new(size) {
            let mut heap = BinaryHeap::from_iter(iter::once(0).chain(iter::once(size)).chain(
                const { 1..N }.map(|_| {
                    let unrestricted = self.prng();
                    let unrestricted = unrestricted as usize;
                    unrestricted % nz
                }),
            ));
            array::from_fn(|_| {
                // SAFETY: Size of `heap` is exactly `N + 1`.
                let lhs = unsafe { heap.pop().unwrap_unchecked() };
                // SAFETY: Size of `heap` is exactly `N + 1`.
                let rhs = *unsafe { heap.peek().unwrap_unchecked() };
                // SAFETY: `heap` is a max-heap, and `rhs` came after `lhs`.
                let difference = unsafe { lhs.unchecked_sub(rhs) };
                (self.split(), difference)
            })
        } else {
            array::from_fn(|_| (self.split(), 0))
        }
    }

    #[inline]
    #[must_use]
    pub const fn prng(&mut self) -> u64 {
        let (prng, seed) = WyRand::gen_u64(self.0);
        self.0 = seed;
        prng
    }

    #[inline]
    #[must_use]
    pub const fn prng_bool(&mut self) -> bool {
        (self.prng() & 1) != 0
    }

    /// With a chance inversely proportional to `size`, stop now;
    /// otherwise, use a stars-and-bars-style subroutine to
    /// split a total size among a known number of children
    /// and generate pseudorandom seeds for each.
    #[inline]
    #[must_use]
    #[expect(
        clippy::as_conversions,
        clippy::cast_possible_truncation,
        reason = "not critical"
    )]
    pub fn should_recurse<const N: usize>(&mut self, size: usize) -> Option<[(Self, usize); N]> {
        // Decrease the remaining size, since
        // an extra node now exists (this one).
        let remaining_size = size.checked_sub(1)?;

        // With a chance inversely proportional to `size`, stop here
        if let Some(nz) = size.checked_add(1)
            // SAFETY: Added 1 above, and overflow already exited.
            && let nz = unsafe { NonZero::new_unchecked(nz) }
            && (self.prng() as usize % nz) == 0
        {
            return None;
        }

        Some(self.partition(remaining_size))
    }

    #[inline]
    #[must_use]
    pub const fn split(&mut self) -> Self {
        Self(self.prng())
    }
}

impl Default for Seed {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}
