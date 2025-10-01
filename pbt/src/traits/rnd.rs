//! Randomly generate instances of this type
//! with a statistically guaranteed weight (in expectation).

use {
    crate::{size::MaybeInstantiable, traits::weight::Weight},
    rand_core::RngCore,
};

#[macro_export]
macro_rules! impl_rnd_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn rnd() {
            #![allow(
                clippy::allow_attributes,
                clippy::as_conversions,
                clippy::cast_precision_loss,
                reason = "not astronomically precise",
            )]

            const N_TRIALS: usize = 10_000;
            const TOLERANCE: f32 = 0.01;

            let max_weight = <$ty as $crate::traits::weight::Weight>::MAX_EXPECTED_WEIGHT;
            let maybe_max_weight = match max_weight {
                $crate::size::MaybeInstantiable::Uninstantiable => {
                    let mut rng = $crate::traits::rnd::default_rng();
                    let rnd = <$ty as $crate::traits::rnd::Rnd>::rnd(&mut rng, 0.);
                    if let $crate::size::MaybeInstantiable::Instantiable(instantiated) = rnd {
                        panic!("Allegedly uninstantiable type was instantiated: `{instantiated:#?}`");
                    }
                    return;
                }
                $crate::size::MaybeInstantiable::Instantiable($crate::size::MaybeInfinite::Infinite) => None,
                $crate::size::MaybeInstantiable::Instantiable($crate::size::MaybeInfinite::Finite(max)) => Some(max),
            };

            assert!(
                matches!(
                    <$ty as $crate::traits::rnd::Rnd>::rnd(&mut $crate::traits::rnd::default_rng(), 0.),
                    $crate::size::MaybeInstantiable::Instantiable(..),
                ),
                "Allegedly instantiable type returned `MaybeInstantiable::Uninstantiable` from `Rnd::rnd`",
            );

            if maybe_max_weight.is_some() {
                let $crate::size::MaybeInstantiable::Instantiable($crate::size::MaybeInfinite::Finite(expected_weight))
                    = <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT // note: NOT expected!
                else {
                    panic!(
                        "Maximum expected was was instantiable and finite but maximum absolute weight was `{:?}`!",
                        <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT,
                    );
                };
                let expected_weight = expected_weight as f32;
                let mean = {
                    let mut rng = $crate::traits::rnd::default_rng();
                    let mut sum: usize = 0;
                    for _trial in 0..N_TRIALS {
                        let $crate::size::MaybeInstantiable::Instantiable(rnd) =
                            <$ty as $crate::traits::rnd::Rnd>::rnd(&mut rng, f32::INFINITY)
                        else {
                            panic!("Allegedly instantiable type returned `MaybeInstantiable::Uninstantiable` from `Rnd::rnd`");
                        };
                        sum += <$ty as $crate::traits::weight::Weight>::weight(&rnd);
                    }
                    (sum as f32) / (N_TRIALS as f32)
                };
                let error = mean - expected_weight;
                let error = error / expected_weight.max(TOLERANCE);
                assert!(error.abs() < TOLERANCE, "Expected the absolute maximum weight, {expected_weight:.1}, but found, on average, {mean:.1} ({:.0}% error)", error * 100.);
            }

            'weights: for expected_weight in [0., 0.5, 1., 2., 5., 10., 50., 100., 1_000., 10_000.] {
                if let Some(max_weight) = maybe_max_weight && expected_weight > max_weight {
                    continue 'weights; // just in case they're out of order in the future, don't `return`
                }
                let mean = {
                    let mut rng = $crate::traits::rnd::default_rng();
                    let mut sum: usize = 0;
                    for _trial in 0..N_TRIALS {
                        let $crate::size::MaybeInstantiable::Instantiable(rnd) =
                            <$ty as $crate::traits::rnd::Rnd>::rnd(&mut rng, expected_weight)
                        else {
                            panic!("Allegedly instantiable type returned `MaybeInstantiable::Uninstantiable` from `Rnd::rnd`");
                        };
                        sum += <$ty as $crate::traits::weight::Weight>::weight(&rnd);
                    }
                    (sum as f32) / (N_TRIALS as f32)
                };
                let error = mean - expected_weight;
                let error = error / expected_weight.max(TOLERANCE);
                assert!(error.abs() < TOLERANCE, "Expected weight {expected_weight} but found, on average, {mean} ({:.0}% error)", error * 100.);
            }
        }
    };
}

// Supposedly higher throughput than a simple 64-bit multiply-and-add,
// plus much, much better qualiy than that, and
// coming from people who know what they're doing.
// Caution: low complexity in lower bits.
// `Xoshiro256**` is an alternative (~15% slowdown).
pub type DefaultRng = rand_xoshiro::Xoshiro256Plus;

/// Randomly generate instances of this type
/// with a statistically guaranteed weight (in expectation).
pub trait Rnd: Weight + Sized {
    /// Randomly generate instances of this type
    /// with a statistically guaranteed weight (in expectation).
    fn rnd<Rng: RngCore>(rng: &mut Rng, expected_weight: f32) -> MaybeInstantiable<Self>;
}

/// A good default random number generator.
#[inline]
#[must_use]
pub fn default_rng() -> DefaultRng {
    <DefaultRng as rand_core::SeedableRng>::seed_from_u64(42)
}
