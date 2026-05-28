//! High-throughput property-based testing with `derive`, swarm-testing, precise sizing,
//! and full graph-theoretic type analysis over mutually inductive and uninstantiable types.

extern crate alloc;

pub mod fields;
pub mod hash;
pub mod impls;
pub mod instantiability;
pub mod multiset;
pub mod pbt;
pub mod reflection;
pub mod scc;
pub mod size;
pub mod swarm;
pub mod unavoidability;
pub mod union_find;

/// Generate an arbitrary term of any type `T`.
///
/// # Errors
///
/// If `T` is uninstantiable.
#[inline]
pub fn arbitrary<T>(
    size: size::Size,
    prng: &mut wyrand::WyRand,
) -> Result<T, reflection::Uninstantiable>
where
    T: pbt::Pbt,
{
    swarm::Swarm::new::<T>(prng)?.arbitrary(size, prng)
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {super::*, core::iter};

    #[test]
    fn deterministic() {
        let mut prng = wyrand::WyRand::new(42);
        let generated: Vec<bool> =
            iter::repeat_with(|| arbitrary(size::Size::zero(), &mut prng).unwrap())
                .take(5)
                .collect();
        let expected = vec![true, false, false, true, false];
        assert_eq!(generated, expected);
    }
}
