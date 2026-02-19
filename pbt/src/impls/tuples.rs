//! Implementations for tuples.

use {
    crate::{
        conjure::{Conjure, ConjureAsync, Seed, Uninstantiable},
        count::{Cardinality, Count},
        shrink::Shrink,
    },
    core::iter,
    futures_util::try_join,
};

macro_rules! impl_corners_for_tuple {
    (;) => {
        iter::once(())
    };
    ($caboose:ident,; $($indices:ident,)*) => {
        $caboose::corners().filter_map(move |$caboose| Some(($($indices::corners().nth($indices)?,)* $caboose,)))
    };
    ($head:ident, $($tail:ident,)*; $($indices:ident,)*) => {
        $head::corners().enumerate().flat_map(move |($head, _)| impl_corners_for_tuple!($($tail,)*; $($indices,)* $head,))
    };
}

macro_rules! impl_shrink_for_tuple {
    ($self:ident, $property:ident,) => {
        None
    };
    ($self:ident, $property:ident, $head:ident, $($tail:ident,)*) => {{
        let ($head, $($tail,)*) = $self.clone();
        let shrunken_head_haha_get_it = $head.step(&mut |$head| $property(&($head::clone($head), $($tail::clone(&$tail),)*)));
        let tail = ($($tail,)*);
        if let Some($head) = shrunken_head_haha_get_it {
            let ($($tail,)*) = tail.step(&mut |&($(ref $tail,)*)| $property(&($head::clone(&$head), $($tail::clone($tail),)*))).unwrap_or(tail);
            Some(($head, $($tail,)*))
        } else {
            tail.step(&mut |&($(ref $tail,)*)| $property(&($head::clone(&$head), $($tail::clone($tail),)*))).map(move |($($tail,)*)| ($head, $($tail,)*))
        }
    }};
}

macro_rules! impl_for_tuple {
    ($($generic:ident),*) => {
        impl<$($generic: Count,)*> Count for ($($generic,)*) {
            const CARDINALITY: Cardinality = {
                let acc = Cardinality::Finite; // for `()`
                $(let acc = acc.of_prod($generic::CARDINALITY);)*
                acc
            };
        }

        #[allow(clippy::allow_attributes, non_snake_case, reason = "until macros are more powerful")]
        impl<$($generic: Conjure,)*> Conjure for ($($generic,)*) {
            #[inline]
            fn conjure(seed: Seed) -> Result<Self, Uninstantiable> {
                let [$($generic,)*] = seed.split();
                Ok(($($generic::conjure($generic)?,)*))
            }

            #[inline]
            fn corners() -> Box<dyn Iterator<Item = Self>> {
                Box::new(impl_corners_for_tuple!($($generic,)*;))
            }

            #[inline]
            fn variants() -> impl Iterator<Item = (Cardinality, fn(Seed) -> Self)> {
                iter::empty()
            }

            #[inline]
            fn leaf(seed: Seed) -> Result<Self, Uninstantiable> {
                let [$($generic,)*] = seed.split();
                Ok(($($generic::leaf($generic)?,)*))
            }
        }

        #[allow(clippy::allow_attributes, non_snake_case, reason = "until macros are more powerful")]
        impl<$($generic: ConjureAsync,)*> ConjureAsync for ($($generic,)*) {
            #[inline]
            async fn conjure_async(seed: Seed) -> Result<Self, Uninstantiable> {
                let [$($generic,)*] = seed.split();
                try_join!($($generic::conjure_async($generic),)*)
            }
        }

        #[allow(clippy::allow_attributes, unused_variables, reason = "for `()`")]
        #[allow(clippy::allow_attributes, non_snake_case, reason = "until macros are more powerful")]
        impl<$($generic: Shrink,)*> Shrink for ($($generic,)*) {
            #[inline]
            fn step<P: FnMut(&Self) -> bool + ?Sized>(&self, property: &mut P) -> Option<Self> {
                impl_shrink_for_tuple!(self, property, $($generic,)*)
            }
        }
    };
}

impl_for_tuple!();
impl_for_tuple!(A);
impl_for_tuple!(A, B);
impl_for_tuple!(A, B, C);
impl_for_tuple!(A, B, C, D);
impl_for_tuple!(A, B, C, D, E);
impl_for_tuple!(A, B, C, D, E, F);
impl_for_tuple!(A, B, C, D, E, F, G);
impl_for_tuple!(A, B, C, D, E, F, G, H);
impl_for_tuple!(A, B, C, D, E, F, G, H, I);
impl_for_tuple!(A, B, C, D, E, F, G, H, I, J);
