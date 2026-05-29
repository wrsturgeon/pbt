//! High-throughput property-based testing with `derive`, swarm-testing, precise sizing,
//! and full graph-theoretic type analysis over mutually inductive and uninstantiable types.

extern crate alloc;

pub mod fields;
pub mod hash;
mod impls;
mod instantiability;
mod multiset;
pub mod reflection;
pub mod registration;
mod scc;
mod size;
mod swarm;
mod unavoidability;
mod union_find;

/// The main property-based testing trait.
pub trait Pbt: 'static + Clone {
    /// Instantiate a specific variant of this type
    /// by providing its index and its fields.
    ///
    /// N.B.: Literal constructors, e.g. on `usize`,
    /// should be instantiated using their built-in `generator` field,
    /// not through this function, since they don't require fields.
    fn construct<F>(parts: reflection::Parts<F>) -> Self
    where
        F: fields::Fields;

    /// Deconstruct a value into its constructor index and its fields.
    fn deconstruct(self) -> reflection::Parts<fields::Store>;

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
    /// # fn f(registration: &mut pbt::registration::Registration<'_>) {
    /// registration.register::<A>();
    /// registration.register::<B>();
    /// registration.register::<C>();
    /// // ... return this type's variants ...
    /// # }
    /// ```
    fn register(registration: &mut registration::Registration<'_>) -> reflection::Reflection<Self>;
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
#[expect(
    clippy::arithmetic_side_effects,
    reason = "The hardware will die before batch size overflows."
)]
pub fn arbitrary<T>(
    prng: &mut wyrand::WyRand,
) -> Result<impl Iterator<Item = T>, reflection::Uninstantiable>
where
    T: Pbt,
{
    let mut swarm_cache = hash::map();
    let mut swarm = swarm::Swarm::new::<T>(prng, &mut swarm_cache)?;
    let mut batch_size = 1_usize; // Increases over time.
    let mut remaining_in_batch = batch_size;
    Ok(size::Size::increasing().map(move |size| {
        if let Some(decremented) = remaining_in_batch.checked_sub(1) {
            remaining_in_batch = decremented;
        } else {
            remaining_in_batch = batch_size;
            batch_size += 1;
            swarm = swarm::Swarm::new::<T>(prng, &mut swarm_cache)
                .expect("INTERNAL ERROR (`pbt`): instantiability changed mid-generation");
        }
        swarm.arbitrary(size, prng)
    }))
}

/// Check that deconstructing and then immediately reconstructing a value is a no-op.
#[inline]
#[expect(
    clippy::absolute_paths,
    reason = "to avoid polluting the top-level namespace"
)]
pub fn check_eta_expansion<T>()
where
    T: Clone + core::fmt::Debug + PartialEq + Pbt,
{
    let mut prng = wyrand::WyRand::new(42);
    let Ok(arbitrary) = arbitrary::<T>(&mut prng) else {
        return;
    };
    for t in arbitrary.take(42) {
        let parts = t.clone().deconstruct();
        let reconstructed = T::construct(parts);
        pretty_assertions::assert_eq!(reconstructed, t);
    }
}
