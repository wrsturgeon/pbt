//! Opaque internal data necessary to
//! register a type with the global type reflection graph.

use {
    crate::{
        Pbt,
        reflection::{Constructors, Erased, ErasedVecOps},
    },
    ahash::HashMap,
    alloc::collections::BTreeMap,
    core::any::TypeId,
};

/// Opaque internal data necessary to
/// register a type with the global type reflection graph.
#[non_exhaustive]
pub struct Registration<'lock> {
    /// Erased function pointers performing operations on vectors of this type.
    pub(crate) erased_vec_ops: &'lock mut HashMap<TypeId, ErasedVecOps>,
    /// The global "naive" variant graph including uninstantiable structures.
    pub(crate) variants: &'lock mut BTreeMap<TypeId, Constructors<Erased>>,
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
            .erased_vec_ops
            .insert(ty, ErasedVecOps::derive::<T>())
            .is_some()
        {
            return;
        }

        // Recurse, i.e. run depth-first search:
        let constructors = T::register(self).erase();
        let dup: Option<_> = self.variants.insert(ty, constructors);
        assert!(
            dup.is_none(),
            "INTERNAL ERROR (`pbt`): TOCTOU despite `&mut` (witchcraft)",
        );
    }
}
