//! Implementations for tuples, up to a finite but large size.
//! If you're using a larger tuple and need this implemented, please open a PR, and I'll almost surely approve it.
//!
//! Implementation inspired by a clever idea [here](https://github.com/BurntSushi/quickcheck/blob/d58e3cffb76fad687318cd1cfc2de165696f6d57/src/arbitrary.rs#L196).

use {
    crate::{
        ast_size::AstSize,
        error,
        exhaust::Exhaust,
        max::{Max, MaybeDecidable, MaybeOverflow},
        pseudorandom::Pseudorandom,
        value_size::ValueSize,
    },
    rand_core::RngCore,
};

/// Count the number of type parameters in a generic tuple.
macro_rules! n_params {
    () => { 0 };
    ($head:ident, $($tail:tt,)*) => {
        1 + n_params!($($tail,)*)
    };
}

/// Implement a specific tuple of generic types.
macro_rules! impl_tuple {
    ($($type_param:ident,)*) => {
        impl<$($type_param: AstSize),*> AstSize for ($($type_param,)*) {
            const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = {
                let acc = MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0_usize)));
                $(
                    let acc = acc.product(&$type_param::MAX_AST_SIZE);
                )*
                acc
            };

            const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> = {
                let acc = MaybeDecidable::Decidable(Max::Finite(0_f32));
                $(
                    let acc = acc.product(&$type_param::MAX_EXPECTED_AST_SIZE);
                )*
                acc
            };

            #[inline]
            fn ast_size(&self) -> MaybeOverflow<usize> {
                #[expect(non_snake_case, reason = "automatic")]
                let ($(ref $type_param,)*) = *self;
                let acc = 0_usize;
                $(
                    let MaybeOverflow::Contained(acc) = $type_param.ast_size().plus(acc) else {
                        return MaybeOverflow::Overflow
                    };
                )*
                MaybeOverflow::Contained(acc)
            }
        }

        impl<$($type_param: ValueSize),*> ValueSize for ($($type_param,)*) {
            const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = {
                let acc = MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0_usize)));
                $(
                    let acc = acc.product(&$type_param::MAX_VALUE_SIZE);
                )*
                acc
            };

            #[inline]
            fn value_size(&self) -> MaybeOverflow<usize> {
                #[expect(non_snake_case, reason = "automatic")]
                let ($(ref $type_param,)*) = *self;
                let acc = 0_usize;
                $(
                    let MaybeOverflow::Contained(acc) = $type_param.value_size().plus(acc) else {
                        return MaybeOverflow::Overflow
                    };
                )*
                MaybeOverflow::Contained(acc)
            }
        }

        impl<$($type_param: Pseudorandom),*> Pseudorandom for ($($type_param,)*) {
            #[inline]
            fn pseudorandom<Rng: RngCore>(
                expected_ast_size: f32,
                rng: &mut Rng,
            ) -> Result<Self, error::Uninstantiable> {
                const N_PARAMS_USIZE: usize = n_params!($($type_param,)*);
                const N_PARAMS: f32 = N_PARAMS_USIZE as f32;

                let even_split = expected_ast_size / N_PARAMS;
                let FullAndPartialFields { n_full_fields, sum_of_partial_sizes } = full_and_partial(
                    even_split,
                    &[$($type_param::MAX_EXPECTED_AST_SIZE,)*],
                );
                // Really, we should iterate until we reach a fixed point,
                // but this will work very well as a good-enough approximation,
                // and the throughput trade-off is a no-brainer.
                let size_per_element = (expected_ast_size - sum_of_partial_sizes) / (n_full_fields as f32);

                Ok(($($type_param::pseudorandom(size_per_element, rng)?,)*))
            }
        }

        /*
        impl<$($type_param: Exhaust),*> Exhaust for ($($type_param,)*) {
            #[inline]
            fn exhaust(value_size: usize) -> Result<impl Iterator<Item = Self>, error::UnreachableSize> {
                const asdf
            }
        }
        */
    };
}

/// Implement every tuple up to and including this full sequence of generic types.
macro_rules! impl_tuples {
    (@internal [$($acc:tt,)*]) => { };
    (@internal [$($acc:tt,)*] $type_param:ident, $($rest:tt,)*) => {
        impl_tuple!($($acc,)* $type_param,);
        impl_tuples!(@internal [$($acc,)* $type_param,] $($rest,)*);
    };
    ($($type_param:ident,)*) => {
        impl_tuples!(@internal [] $($type_param,)*);
    };
}

impl_tuples!(
    T1, T2, T3, T4, T5, T6, T7, T8, T9, T10, T11, T12, T13, T14, T15,
    T16, // T17, T18, T19, T20, T21, T22, T23, T24, T25, T26, T27, T28, T29, T30, T31, T32,
);

/// Let's say we're iterating over fields in a tuple,
/// and we know the maximum size of each field (unless undecidable, overflowed, etc.).
/// Let's further suppose that we have an overall goal size in mind for the whole tuple.
/// Then if we split the size evenly over all fields, it would be applied unevenly in general:
/// some fields are uninstantiable, and some have a finite size lower than the even split.
struct FullAndPartialFields {
    n_full_fields: usize,
    sum_of_partial_sizes: f32,
}

#[inline]
pub const fn full_and_partial(
    even_split: f32,
    max_expected_ast_sizes: &[MaybeDecidable<Max<f32>>],
) -> FullAndPartialFields {
    match *max_expected_ast_sizes {
        [] => FullAndPartialFields {
            n_full_fields: 0,
            sum_of_partial_sizes: 0.,
        },
        [ref head, ref tail @ ..] => {
            let mut rec = full_and_partial(even_split, tail);
            match *head.at_most() {
                Max::Uninstantiable => {}
                Max::Infinite => rec.n_full_fields += 1,
                Max::Finite(max) => {
                    if max >= even_split {
                        rec.n_full_fields += 1
                    } else {
                        rec.sum_of_partial_sizes += max
                    }
                }
            }
            rec
        }
    }
}
