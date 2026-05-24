//! Built-in Rust `TypeId`s with better ergonomics,
//! since e.g. we have type names available at runtime.

use {
    crate::reflection,
    core::{
        any::{self, TypeId},
        fmt,
    },
};

/// Built-in Rust `TypeId`s with better ergonomics,
/// since e.g. we have type names available at runtime.
#[derive(Clone, Copy, Eq, Ord, Hash, PartialEq, PartialOrd)]
pub struct Type {
    /// The built-in Rust `TypeId` for this type.
    id: TypeId,
}

impl Type {
    /// Erase this type into a unique ID at runtime.
    #[inline]
    #[must_use]
    pub const fn new<T>() -> Self
    where
        T: 'static + ?Sized,
    {
        Self {
            id: any::TypeId::of::<T>(),
        }
    }
}

impl fmt::Debug for Type {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match reflection::get_immediate(self) {
            Some(adt) => write!(f, "`{}`", adt.name),
            None => write!(f, "<inaccessible type with ID {:?}>", self.id),
        }
    }
}

impl fmt::Display for Type {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <Self as fmt::Debug>::fmt(self, f)
    }
}
