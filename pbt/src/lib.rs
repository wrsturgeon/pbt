//! High-throughput property-based testing with `derive`, swarm-testing, precise sizing,
//! and full graph-theoretic type analysis over mutually inductive and uninstantiable types.

extern crate alloc;

mod fields;
pub mod hash;
mod impls;
mod instantiability;
mod multiset;
pub mod reflection;
mod scc;
mod size;
mod swarm;
mod unavoidability;
mod union_find;

/// The main property-based testing trait.
#[expect(
    clippy::absolute_paths,
    reason = "to avoid polluting the top-level namespace"
)]
pub trait Pbt: 'static {
    /// Instantiate a specific variant of this type
    /// by providing its index and its fields.
    ///
    /// N.B.: Literal constructors, e.g. on `usize`,
    /// should be instantiated using their built-in `generator` field,
    /// not through this function, since they don't require fields.
    fn instantiate_variant<F>(variant_index: usize, fields: F) -> Self
    where
        F: fields::Fields;

    /// Enumerate the logical structure of all variants of this type.
    ///
    /// This must *also* register all dependencies of this type.
    /// For example, if this type  contains fields of types
    /// `A`, `B`, and `C`,  we'd write the following:
    /// ```rust
    /// # extern crate alloc;
    /// # type A = bool;
    /// # type B = usize;
    /// # type C = usize;
    /// # let mut variants_map = alloc::collections::BTreeMap::new();
    /// # let mut visited_map = pbt::hash::set();
    /// # let variants = &mut variants_map;
    /// # let visited = &mut visited_map;
    /// pbt::reflection::register::<A>(variants, visited);
    /// pbt::reflection::register::<B>(variants, visited);
    /// pbt::reflection::register::<C>(variants, visited);
    /// // ... return this type's variants ...
    /// ```
    fn variants(
        variants: &mut alloc::collections::BTreeMap<
            core::any::TypeId,
            alloc::sync::Arc<[reflection::Constructor<reflection::Erased>]>,
        >,
        visited: &mut ahash::HashSet<core::any::TypeId>,
    ) -> Vec<reflection::Variant<Self>>;
}

/// Generate an arbitrary term of any type `T`.
///
/// # Errors
///
/// If `T` is uninstantiable.
#[inline]
#[expect(
    clippy::expect_used,
    clippy::missing_panics_doc,
    reason = "Internal invariants: violations should fail loudly."
)]
pub fn arbitrary<T>(
    prng: &mut wyrand::WyRand,
) -> Result<impl Iterator<Item = T>, reflection::Uninstantiable>
where
    T: Pbt,
{
    let mut swarm_cache = hash::map();
    let _check_instantiability_and_warm_cache = swarm::Swarm::new::<T>(prng, &mut swarm_cache)?;
    Ok(size::Size::increasing().map(move |size| {
        swarm::Swarm::new::<T>(prng, &mut swarm_cache)
            .expect("INTERNAL ERROR (`pbt`): instantiability mismatch")
            .arbitrary(size, prng)
    }))
}
