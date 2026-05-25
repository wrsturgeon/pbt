//! The main property-based testing trait.

use {
    crate::{
        reflection::{Reflection, TypeGraphVertex},
        type_id::Type,
    },
    ahash::{HashMap, HashSet},
    alloc::sync::Arc,
};

/// The main property-based testing trait.
pub trait Pbt: 'static {
    /// Type-level reflection: variants, field types, erased trait operations, etc.
    ///
    /// This must *also* register all dependencies of this type:
    /// specifically, for each type `T` of each field of each variant,
    /// this function must call `::pbt::reflection::register::<T>(vertices, visited)`.
    fn reflect(
        vertices: &mut HashMap<Type, Arc<TypeGraphVertex>>,
        visited: &mut HashSet<Type>,
    ) -> Reflection<Self>;
}
