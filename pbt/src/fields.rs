//! Logic for generating and/or storing
//! fields to be used on a given constructor.

use {
    crate::{
        Pbt,
        hash::map,
        multiset::Multiset,
        reflection::Erased,
        size::{self, Size},
        swarm::Swarm,
    },
    ahash::HashMap,
    core::{any::TypeId, mem, ptr},
    std::collections::hash_map,
    wyrand::WyRand,
};

/// Logic for generating and/or storing
/// fields to be used on a given constructor.
///
/// Note that this unifies two cases:
/// generation, in which we want fields on demand with maximum throughput,
/// and shrinking, in which we want to reuse existing fields.
pub trait Fields {
    /// Retrieve and/or generate a term of type T.
    fn field<T>(&mut self) -> T
    where
        T: Pbt;
}

/// Fields are not stored ahead of time;
/// instead, their sizes are stored in an iterator,
/// and all fields are produced just in time.
#[non_exhaustive]
pub(crate) struct Lazy<'prng, 'swarm> {
    /// Pseudorandom number generator.
    ///
    /// This is inside `Lazy` and not a function argument
    /// because shrinking (existing fields) doesn't need a PRNG.
    pub(crate) prng: &'prng mut WyRand,
    /// A lazy partition over sizes, tuned to match
    /// the number of inductive types among the fields to generate.
    pub(crate) sizes: size::Partition,
    /// A masked view into this type's constructors,
    /// partitioned into potential leaves and loops.
    pub(crate) swarm: &'swarm Swarm,
}

/// A collection of fields of arbitrary/mixed types.
/// Fields are known and returned if present;
/// unknown fields are newly generated leaves.
#[non_exhaustive]
pub struct Store {
    /// A map from type IDs to erased vectors
    /// whose elements match the associated type.
    store: HashMap<TypeId, Vec<Erased>>,
}

impl Fields for Lazy<'_, '_> {
    #[inline]
    fn field<T>(&mut self) -> T
    where
        T: Pbt,
    {
        let size = if self.swarm.is_inductive::<T>() {
            // SAFETY: `Partition::next` always returns `Some(_)`,
            // since it returns endless zeros after its assigned cardinality.
            unsafe { self.sizes.next().unwrap_unchecked() }
        } else {
            Size::zero()
        };
        self.swarm.arbitrary(size, self.prng)
    }
}

impl Fields for Store {
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    fn field<T>(&mut self) -> T
    where
        T: Pbt,
    {
        self.pop().expect("INTERNAL ERROR (`pbt`): missing field")
    }
}

impl Store {
    /// An empty collection of fields of arbitrary/mixed types.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { store: map() }
    }

    /// Pop and return a cached field of this type iff one exists.
    #[inline]
    pub(crate) fn pop<T>(&mut self) -> Option<T>
    where
        T: 'static,
    {
        let ty = TypeId::of::<T>();
        let hash_map::Entry::Occupied(mut entry) = self.store.entry(ty) else {
            return None;
        };
        let erased: &mut Vec<Erased> = entry.get_mut();
        // SAFETY: Invariant. Extremely dangerous.
        let typed: &mut Vec<T> =
            unsafe { ptr::from_mut(erased).cast::<Vec<T>>().as_mut_unchecked() };
        let t = typed.pop()?;
        if typed.is_empty() {
            let erased_to_drop: Vec<Erased> = entry.remove();
            // SAFETY: Invariant. Extremely dangerous.
            let typed_to_drop: Vec<T> =
                unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(erased_to_drop) };
            let () = drop(typed_to_drop);
        }
        Some(t)
    }

    /// Store a field of this type.
    #[inline]
    pub fn push<T>(&mut self, t: T)
    where
        T: 'static,
    {
        let ty = TypeId::of::<T>();
        let erased: &mut Vec<Erased> = self.store.entry(ty).or_default();
        // SAFETY: Invariant. Extremely dangerous.
        let typed: &mut Vec<T> =
            unsafe { ptr::from_mut(erased).cast::<Vec<T>>().as_mut_unchecked() };
        typed.push(t);
    }

    /// The multiset of types stored.
    #[inline]
    #[must_use]
    pub fn types(&self) -> Multiset<TypeId> {
        self.store.iter().map(|(&k, v)| (k, v.len())).collect()
    }
}

impl Default for Store {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Store {
    #[inline]
    fn drop(&mut self) {
        assert!(
            self.store.is_empty(),
            "INTERNAL ERROR (`pbt`): unused fields (can't drop while type-erased!)",
        );
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "Failing tests ought to panic.")]

    use {super::*, crate::arbitrary, core::iter, pretty_assertions::assert_eq};

    // TODO: make this a real PBT when macro are ready
    #[test]
    fn lossless() {
        let mut prng = WyRand::new(42);
        for ints in arbitrary::<Vec<usize>>(&mut prng).unwrap().take(10) {
            let mut store = Store::new();
            for &int in ints.iter().rev() {
                let () = store.push(int);
            }
            let reconstructed: Vec<usize> = iter::from_fn(|| store.pop()).collect();
            assert_eq!(reconstructed, ints);
        }
    }

    #[test]
    fn types() {
        let mut store = Store::new();
        let () = store.push(42_usize);
        let () = store.push(true);
        let () = store.push(false);
        let () = store.push(42_usize); // <-- duplicate
        let () = store.push(vec![false]);
        let expected: Multiset<TypeId> = [
            (TypeId::of::<usize>(), 2),
            (TypeId::of::<bool>(), 2),
            (TypeId::of::<Vec<bool>>(), 1),
        ]
        .into_iter()
        .collect();
        assert_eq!(store.types(), expected);
        let _: usize = store.pop().unwrap();
        let _: usize = store.pop().unwrap();
        let _: bool = store.pop().unwrap();
        let _: bool = store.pop().unwrap();
        let _: Vec<bool> = store.pop().unwrap();
    }
}
