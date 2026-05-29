//! Opaque internal data necessary to
//! register a type with the global type reflection graph.

use {
    crate::{
        Pbt,
        reflection::{BucketOps, Constructor, Erased},
    },
    ahash::HashMap,
    alloc::{collections::BTreeMap, sync::Arc},
    core::{any::TypeId, mem},
};

/// Opaque internal data necessary to
/// register a type with the global type reflection graph.
#[non_exhaustive]
pub struct Registration<'lock> {
    /// Erased function pointers performing operations on vectors of this type.
    pub(crate) bucket_ops: &'lock mut HashMap<TypeId, BucketOps<Erased>>,
    /// The global "naive" variant graph including uninstantiable structures.
    pub(crate) variants: &'lock mut BTreeMap<TypeId, Arc<[Constructor<Erased>]>>,
}

impl Registration<'_> {
    /// Register the type `T` and its dependencies
    /// in a naive type reflection graph,
    /// including any uninstantiable variants.
    #[inline]
    #[expect(
        clippy::missing_panics_doc,
        reason = "Internal invariants: violations should fail loudly."
    )]
    pub fn register<T>(&mut self)
    where
        T: Pbt,
    {
        // If this type has already been registered, short-circuit:
        let ty = TypeId::of::<T>();
        if self
            .bucket_ops
            .insert(ty, BucketOps::<T>::new().erase())
            .is_some()
        {
            return;
        }

        // Recurse, i.e. run depth-first search:
        let ordered_naive_variants = T::register(self);
        let naive_variants = ordered_naive_variants
            .variants
            .into_iter()
            .enumerate()
            .map(|(index, variant)| Constructor { index, variant })
            .collect();

        // SAFETY: `T` is only ever the codomain of a function pointer.
        let erased = unsafe {
            mem::transmute::<
                Arc<[Constructor<T>]>, //
                Arc<[Constructor<Erased>]>,
            >(naive_variants)
        };

        let dup: Option<_> = self.variants.insert(ty, erased);
        assert!(
            dup.is_none(),
            "INTERNAL ERROR (`pbt`): TOCTOU despite `&mut` (witchcraft)",
        );
    }
}
