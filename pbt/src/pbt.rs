//! The main property-based testing trait.

use {
    crate::reflection::{Erased, Variant},
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
    core::any::TypeId,
};

/// The main property-based testing trait.
pub trait Pbt: 'static {
    /// Enumerate the logical structure of all variants of this type.
    ///
    /// This must *also* register all dependencies of this type.
    /// For example, if this type  contains fields of types
    /// `A`, `B`, and `C`,  we'd write the following:
    /// ```rust
    /// # type A = bool;
    /// # type B = usize;
    /// # type C = usize;
    /// # let mut variants_map = pbt::hash::map();
    /// # let mut visited_map = pbt::hash::set();
    /// # let variants = &mut variants_map;
    /// # let visited = &mut visited_map;
    /// pbt::reflection::register::<A>(variants, visited);
    /// pbt::reflection::register::<B>(variants, visited);
    /// pbt::reflection::register::<C>(variants, visited);
    /// // ... return this type's variants ...
    /// ```
    fn variants(
        variants: &mut HashMap<TypeId, Arc<[Variant<Erased>]>>,
        visited: &mut HashSet<TypeId>,
    ) -> Arc<[Variant<Self>]>;
}
