//! High-throughput property-based testing with `derive`, swarm-testing, precise sizing,
//! and full graph-theoretic type analysis over mutually inductive and uninstantiable types.

extern crate alloc;

mod arbitrary;
pub mod fields;
pub mod hash;
mod impls;
mod instantiability;
pub mod multiset;
pub mod panic;
mod persist;
pub mod reflection;
pub mod registration;
mod scc;
mod shrink;
mod size;
mod swarm;
mod unavoidability;
mod union_find;

pub use {
    pbt_macros::{Pbt, pbt},
    wyrand::WyRand,
};

/// The default number of cases to check if no alternate is specified.
#[cfg(not(miri))]
pub const DEFAULT_N_CASES: usize = 10_000;

/// The default number of cases to check if no alternate is specified.
#[cfg(miri)]
pub const DEFAULT_N_CASES: usize = 100;

/// The main property-based testing trait.
#[expect(
    clippy::absolute_paths,
    reason = "to avoid polluting the top-level namespace"
)]
pub trait Pbt: 'static + Clone + core::fmt::Debug {
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
    fn register(registration: &mut registration::Registration<'_>) -> reflection::Variants<Self>;
}

/// Check that deconstructing and then immediately reconstructing a value is a no-op.
#[inline]
pub fn check_eta_expansion<T>()
where
    T: PartialEq + Pbt,
{
    let mut prng = wyrand::WyRand::new(getrandom());
    let Ok(arbitrary) = arbitrary::arbitrary::<T>(&mut prng) else {
        return;
    };
    for t in arbitrary.take(DEFAULT_N_CASES >> 2) {
        let parts = t.clone().deconstruct();
        let reconstructed = T::construct(parts);
        pretty_assertions::assert_eq!(
            reconstructed,
            t,
            "\r\n\r\n{t:?} -> Parts {{ .. }} -> {reconstructed:?} =/= {t:?}",
        );
    }
}

/// Check that serializing and then immediately deserializing a value is a no-op.
#[inline]
pub fn check_serialization<T>()
where
    T: PartialEq + Pbt,
{
    let mut prng = wyrand::WyRand::new(getrandom());
    let Ok(arbitrary) = arbitrary::arbitrary::<T>(&mut prng) else {
        return;
    };
    for t in arbitrary.take(DEFAULT_N_CASES >> 2) {
        let json = t.clone().deconstruct().serialize();
        let reconstructed: Option<T> = reflection::Parts::deserialize(&json);
        pretty_assertions::assert_eq!(
            reconstructed,
            Some(t.clone()),
            "\r\n\r\n{t:?} -> {json:?} -> {reconstructed:?} =/= Some({t:?})",
        );
    }
}

/// Get a(n expensive) random `u64` from the OS via the `getrandom` crate.
///
/// # Panics
///
/// If and only if the `getrandom` crate panics.
#[inline]
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub fn getrandom() -> u64 {
    getrandom::u64().expect("INTERNAL ERROR (`pbt`): `getrandom` failed")
}

/// Search for the smallest witness of an arbitrary property, if one exists.
///
/// If this fails, this does not mean that the property never holds;
/// instead, it simply means we didn't find a property in `cases` cases.
#[inline]
pub fn witness<T, Property, Proof>(
    property: Property,
    cases: usize,
    prng: &mut wyrand::WyRand,
) -> Option<(T, Proof)>
where
    Property: Fn(&T) -> Option<Proof>,
    T: Pbt,
{
    let arbitrary = arbitrary::arbitrary::<T>(prng).ok()?;
    for t in arbitrary.take(cases) {
        if let Some(proof) = property(&t) {
            return Some(shrink::to_minimal_witness(&property, t, proof));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use {super::*, pretty_assertions::assert_eq, wyrand::WyRand};

    #[test]
    fn witness_at_least_42() {
        let mut prng = WyRand::new(42); // deterministic
        assert_eq!(
            witness(|i: &usize| i.checked_sub(42), DEFAULT_N_CASES, &mut prng),
            Some((42, 0))
        );
    }
}
