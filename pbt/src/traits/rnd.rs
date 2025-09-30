//! Randomly generate instances of this type
//! with a statistically guaranteed weight (in expectation).

use {crate::size::MaybeInstantiable, rand_core::RngCore};

#[macro_export]
macro_rules! impl_rnd_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn rnd() {
            let max_weight = <$ty as $crate::traits::weight::Weight>::MAX_WEIGHT;
            match max_weight {
                $crate::size::MaybeInstantiable::Uninstantiable => {
                    let mut rng = $crate::traits::rnd::default_rng();
                    let rnd = <$ty as $crate::traits::rnd::Rnd>::rnd(&mut rng, 0);
                    if let $crate::size::MaybeInstantiable::Instantiable(instantiated) = rnd {
                        panic!("Allegedly uninstantiable type was instantiated: `{instantiated:#?}`");
                    }
                }
                $crate::size::MaybeInstantiable::Instantiable(..) => {
                    let mut rng = $crate::traits::rnd::default_rng();
                    let rnd = <$ty as $crate::traits::rnd::Rnd>::rnd(&mut rng, 0);
                    assert!(
                        matches!(rnd, $crate::size::MaybeInstantiable::Instantiable(..)),
                        "Allegedly instantiable type returned `MaybeInstantiable::Uninstantiable` from `Rnd::rnd`",
                    );
                }
            }

            #[allow(
                clippy::allow_attributes,
                clippy::as_conversions,
                clippy::cast_precision_loss,
                reason = "not astronomically precise",
            )]
            for expected_weight in [1, 10, 100, 1_000, 10_000] {
                if $crate::size::MaybeInstantiable::Instantiable($crate::size::MaybeInfinite::Finite(expected_weight)) > max_weight {
                    return;
                }
                let mean = {
                    const N_TRIALS: usize = 10_000;

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
                let error = mean / (expected_weight as f32);
                let error = error - 1.;
                assert!(error.abs() < 0.01, "Expected weight {expected_weight} but found, on average, {mean} ({:4.0}% error)", error * 100.);
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
pub trait Rnd: Sized {
    /// Randomly generate instances of this type
    /// with a statistically guaranteed weight (in expectation).
    fn rnd<Rng: RngCore>(rng: &mut Rng, expected_weight: usize) -> MaybeInstantiable<Self>;
}

/// A good default random number generator.
#[inline]
#[must_use]
pub fn default_rng() -> DefaultRng {
    <DefaultRng as rand_core::SeedableRng>::seed_from_u64(42)
}
