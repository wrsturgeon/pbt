//! Implementations for `Option<_>`.

use {
    crate::{
        either::Either,
        size::{MaybeInfinite, MaybeInstantiable, MaybeOverflow},
        stats::bernoulli,
        traits::{
            corner::Corner, decimate::Decimate, refine::Refine, rnd::Rnd, size::Size,
            weight::Weight,
        },
    },
    core::{hint::unreachable_unchecked, iter, option},
};

impl<T: Weight> Weight for Option<T> {
    const MAX_EXPECTED_WEIGHT: MaybeInstantiable<MaybeInfinite<f32>> = match T::MAX_EXPECTED_WEIGHT
    {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0.))
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite)) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(
                // TODO: educated guess!
                (finite + 1.) / (finite + 2.),
            ))
        }
    };
    const MAX_WEIGHT: MaybeInstantiable<MaybeInfinite<usize>> = match T::MAX_WEIGHT {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(0))
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite)) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite + 1))
        }
    };
    #[inline]
    fn weight(&self) -> usize {
        self.as_ref().map_or(0, |some| {
            // SAFETY: Any memory location can fit in a `usize`, and
            // `Weight` measures only size that takes up memory,
            // so since the original value was representable in memory,
            // its weight will fit in a `usize`.
            unsafe { some.weight().unchecked_add(1) }
        })
    }
}

impl<T: Size> Size for Option<T> {
    const MAX_SIZE: MaybeInstantiable<MaybeInfinite<MaybeOverflow<usize>>> = match T::MAX_SIZE {
        MaybeInstantiable::Uninstantiable => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(MaybeOverflow::Contained(0)))
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Infinite)
        }
        MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite)) => {
            MaybeInstantiable::Instantiable(MaybeInfinite::Finite(finite.plus(1)))
        }
    };
    #[inline]
    fn size(&self) -> MaybeOverflow<usize> {
        self.as_ref()
            .map_or(MaybeOverflow::Contained(0), |some| some.size().plus(1))
    }
}

impl<T: Corner> Corner for Option<T> {
    type Corners = iter::Chain<iter::Once<Self>, iter::Map<T::Corners, fn(T) -> Self>>;
    #[inline]
    fn corners() -> Self::Corners {
        iter::once(None).chain(T::corners().map(
            #[expect(
                clippy::as_conversions,
                reason = "Function pointer conversions are checked more thoroughly"
            )]
            {
                Some as fn(_) -> _
            },
        ))
    }
}

impl<T: Rnd> Rnd for Option<T> {
    #[inline]
    fn rnd<Rng: rand_core::RngCore>(
        rng: &mut Rng,
        expected_weight: f32,
    ) -> MaybeInstantiable<Self> {
        // Let's work out the expectation so we can set it equal and work backward.
        // Note that we can choose the expected weight of `T` in `Some(..)`,
        // so we can choose any value for `Pr[None]` and compensate for it in `Some(..)`.
        // I'd like `Pr[None]` to be inversely proportional to `expected_weight`:
        // the smallest power (-1) at which the integral diverges, so there's always
        // a small but non-negligible chance of choosing `None`, no matter how large.
        // Since `expected_weight` can be less than `1` and probabilities are on `[0, 1]`,
        // let's use `1 / (1 + expected_weight)` instead of the naive reciprocal:
        // that way, when `expected_weight` is `0`, we're guaranteed to choose `None`.
        // The complement, `Pr[Some(..)]`, is `1 - 1 / (1 + expected_weight)`,
        // which can be rearranged to `expected_weight / (1 + expected_weight)`.
        // So we need to set the expected weight of the `T` in `Some(..)` to some `W`
        // such that `(expected_weight / (expected_weight + 1)) (1 + W) = expected_weight`:
        // note that the weight of `None` is `0` (so we can drop it in weight calculations)
        // and choosing `Some(..)` incurs a weight penalty of `1` (hence `1 + W`).
        // We can divide both sides by `expected_weight`: `(1 + W) / (expected_weight + 1) = 1`.
        // Then multiply both sides by `expected_weight + 1`: `1 + W = expected_weight + 1`.
        // Then subtract both sides by `1`: `W = expected_weight`.
        // Huh. That was easy!
        // Now what about the case of a finite `T` (up to, say, `F`)?
        // If we keep the inversely proportional `Pr[None]`, then we have
        // `(1 + min(W, F)) / (expected_weight + 1) = 1 ==> min(W, F) = expected_weight`,
        // which isn't really actionable when `expected_weight > F`.
        // So we need to tweak `Pr[None]`--but let's phrase it as tweaking `Pr[Some(..)]` instead.
        // So `Pr[Some(..)] (1 + min(W, F)) = expected_weight`.
        // This one's not as pretty: `Pr[Some(..)] = expected_weight / (1 + min(W, F))`.
        // I'm going to assume that `W = expected_weight` here, since that's optimal otherwise.
        // So, final answer: `Pr[Some(..)] = expected_weight / (1 + min(expected_weight, F))`.

        let pr_some = {
            let expected_some_weight = match T::MAX_EXPECTED_WEIGHT {
                MaybeInstantiable::Uninstantiable => {
                    return MaybeInstantiable::Instantiable(None);
                }
                MaybeInstantiable::Instantiable(MaybeInfinite::Infinite) => expected_weight,
                MaybeInstantiable::Instantiable(MaybeInfinite::Finite(max)) => {
                    expected_weight.min(max)
                }
            };
            expected_weight / (1. + expected_some_weight)
        };

        MaybeInstantiable::Instantiable(bernoulli(rng, pr_some).then(move || {
            let MaybeInstantiable::Instantiable(some) = T::rnd(rng, expected_weight) else {
                // SAFETY:
                // If `T` were uninstantiable, we would have `return`ed in the above `match`.
                unsafe { unreachable_unchecked() }
            };
            some
        }))
    }
}

impl<T: Decimate> Decimate for Option<T> {
    type Decimate = Either<
        iter::Flatten<option::IntoIter<iter::Map<T::Decimate, fn(T) -> Self>>>,
        iter::Once<Self>,
    >;
    #[inline]
    fn decimate(&self, weight: usize) -> Self::Decimate {
        weight
            .checked_sub(1)
            .map_or_else(
                || Either::B(iter::once(None)),
                |weight| {
                    Either::A(
                        self.as_ref()
                            .map(|some| {
                                some.decimate(weight).map(
                                    #[expect(
                                        clippy::as_conversions,
                                        reason = "Function pointer conversions are checked more thoroughly"
                                    )]
                                    {
                                        Some as fn(_) -> _
                                    },
                                )
                            })
                            .into_iter()
                            .flatten(),
                    )
                },
            )
    }
}

impl<T: Refine> Refine for Option<T> {
    type Refine = Either<iter::Map<T::Refine, fn(T) -> Self>, option::IntoIter<Self>>;
    #[inline]
    fn refine(&self, size: usize) -> Self::Refine {
        self.as_ref().map_or_else(
            || Either::B(Some(None).into_iter()),
            |some| {
                size.checked_sub(1).map_or_else(
                    || Either::B(None.into_iter()),
                    |size| {
                        Either::A(some.refine(size).map(
                            #[expect(
                                clippy::as_conversions,
                                reason = "Function pointer conversions are checked more thoroughly"
                            )]
                            {
                                Some as fn(_) -> _
                            },
                        ))
                    },
                )
            },
        )
    }
}

#[cfg(test)]
mod test {
    use {super::*, core::convert::Infallible};

    crate::impl_tests!(Option<Infallible>, option_void);
    crate::impl_tests!(Option<()>, option_unit);
    crate::impl_tests!(Option<bool>, option_bool); // TODO: remove and switch to the below
    // crate::impl_tests!(Option<u8>, option_u8); // TODO
}
