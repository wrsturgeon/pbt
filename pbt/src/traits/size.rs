//! The precise, detailed size of a value, including any `Copy` values.

use crate::size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow};

#[macro_export]
macro_rules! impl_size_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn max_size() {
            let finite = match <$ty as $crate::traits::size::Size>::MAX_SIZE {
                $crate::size::MaybeInstantiable::Uninstantiable => {
                    assert!(
                        matches!(
                            <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT,
                            $crate::size::MaybeInstantiable::Uninstantiable,
                        ),
                        "Expected weight and size to agree on instantiability and finiteness, but size (`{:#?}`) =/= weight (`{:#?}`)",
                        <$ty as $crate::traits::size::Size>::MAX_SIZE,
                        <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT,
                    );
                    if let Some(corner) = <$ty as $crate::traits::corner::Corner>::corners().next() {
                        panic!("Expected an uninstantiable type but found a corner case: {corner:#?}");
                    }
                }
                $crate::size::MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
                    assert!(
                        matches!(
                            <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT,
                            $crate::size::MaybeInstantiable::Instantiable(MaybeInfinite::Infinite),
                        ),
                        "Expected weight and size to agree on instantiability and finiteness, but size (`{:#?}`) =/= weight (`{:#?}`)",
                        <$ty as $crate::traits::size::Size>::MAX_SIZE,
                        <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT,
                    );
                    assert!(
                        <$ty as $crate::traits::corner::Corner>::corners().next().is_some(),
                        "Expected an infinitely instantiable type but found no corner cases",
                    );
                }
                $crate::size::MaybeInstantiable::Instantiable(MaybeInfinite::Finite(max)) => {
                    assert!(
                        matches!(
                            <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT,
                            $crate::size::MaybeInstantiable::Instantiable(MaybeInfinite::Finite(..)),
                        ),
                        "Expected weight and size to agree on instantiability and finiteness, but size (`{:#?}`) =/= weight (`{:#?}`)",
                        <$ty as $crate::traits::size::Size>::MAX_SIZE,
                        <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT,
                    );
                    assert!(
                        <$ty as $crate::traits::corner::Corner>::corners().next().is_some(),
                        "Expected a finitely instantiable type but found no corner cases",
                    );
                    for corner in <$ty as $crate::traits::corner::Corner>::corners() {
                        let size = <$ty as $crate::traits::size::Size>::size(&corner);
                        assert!(size <= max, "Expected a maximum size of {max:?}, but the corner-case `{corner:#?}` has size {size:?}");
                    }
                }
            };
        }
    };
}

/// The precise, detailed size of a value, including any `Copy` values.
pub trait Size {
    /// The maximum value that can ever be returned by `self.size()`,
    /// if any, over all values for `self: Self`.
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>>;
    /// The precise, detailed size of a value, including any `Copy` values.
    fn size(&self) -> MaybeOverflow<usize>;
}
