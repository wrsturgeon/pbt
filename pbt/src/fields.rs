//! Logic for generating and/or storing
//! fields to be used on a given constructor.

use {
    crate::{
        Pbt,
        size::{self, Size},
        swarm::Swarm,
    },
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
#[expect(clippy::field_scoped_visibility_modifiers, reason = "fine")]
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
    pub(crate) swarm: &'swarm mut Swarm,
}

/// Fields are known and returned if present;
/// unknown fields are newly generated leaves.
#[non_exhaustive]
#[expect(clippy::empty_structs_with_brackets, reason = "TODO")]
pub struct Eager {
    // TODO: erased bag of type-indexed terms
}

impl Fields for Lazy<'_, '_> {
    #[inline(always)]
    #[expect(
        clippy::expect_used,
        reason = "For internal use only: invariant violations should fail loudly."
    )]
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
        self.swarm
            .arbitrary(size, self.prng)
            .expect("INTERNAL ERROR (`pbt`): uninstantiable field")
    }
}
