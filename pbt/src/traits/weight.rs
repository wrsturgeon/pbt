//! The coarse, overall size of a value, skipping all `Copy` values.

use crate::size::{MaybeInfinite, MaybeInstantiable};

/// Test compliance with the crucial invariants assumed of `pbt::Weight`.
#[macro_export]
macro_rules! impl_weight_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn max_weight() {
            // Make sure, as a baseline, it at least doesn't panic:
            for corner in <$ty as $crate::traits::corner::Corner>::corners() {
                let _: usize = <$ty as $crate::traits::weight::Weight>::weight(&corner);
            }

            // Then enforce consistency with stated maxima:
            match <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT {
                $crate::size::MaybeInstantiable::Uninstantiable => {
                    if let Some(corner) = <$ty as $crate::traits::corner::Corner>::corners().next() {
                        panic!("Expected an uninstantiable type but found a corner case: {corner:#?}");
                    }
                }
                $crate::size::MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
                    assert!(
                        <$ty as $crate::traits::corner::Corner>::corners().next().is_some(),
                        "Expected an infinitely instantiable type but found no corner cases",
                    );
                }
                $crate::size::MaybeInstantiable::Instantiable(MaybeInfinite::Finite(max)) => {
                    assert!(
                        <$ty as $crate::traits::corner::Corner>::corners().next().is_some(),
                        "Expected a finitely instantiable type but found no corner cases",
                    );
                    for corner in <$ty as $crate::traits::corner::Corner>::corners() {
                        let weight = <$ty as $crate::traits::weight::Weight>::weight(&corner);
                        assert!(
                            weight <= max,
                            "Expected a maximum weight of {max:?}, but the corner-case `{corner:#?}` has weight {weight:?}",
                        );
                    }
                }
            }
        }
    };
}

/// The coarse, overall size of a value, skipping all `Copy` values.
pub trait Weight {
    // TODO: remove?
    /// The maximum value that can ever be *expected* to be returned by `self.weight()`,
    /// if any, over all values for `self: Self`.
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>>;
    /// The maximum value that can ever be returned by `self.weight()`,
    /// if any, over all values for `self: Self`.
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>>;
    /// The coarse, overall size of a value, skipping all `Copy` values.
    /// # Overflow
    /// While overflow *is* possible in the `Size` trait,
    /// it is logically not possible here (when implemented correctly),
    /// since any memory location can fit in a `usize` without overflow,
    /// and this should measure only structural size,
    /// i.e. that which takes up extra space in memory.
    fn weight(&self) -> usize;
}
