//! Implementations for `(_, _)`.

use {
    crate::{
        Pbt,
        fields::{Fields, Store},
        reflection::{Parts, Variant, Variants},
        registration::Registration,
    },
    core::{any::TypeId, num::NonZero},
};

/// Implement `Pbt` for a generic tuple of types.
macro_rules! impl_for_tuple {
    ($($id:ident,)*) => {
        #[allow(
            non_snake_case,
            unused_mut,
            unused_variables,
            clippy::allow_attributes,
            reason = "automatically generated"
        )]
        impl<$($id,)*> Pbt for ($($id,)*)
        where
            $($id: Pbt,)*
        {
            #[inline]
            fn construct<F>(
                Parts {
                    mut fields,
                    variant_index,
                }: Parts<F>,
            ) -> Self
            where
                F: Fields,
            {
                let algebraic_index: usize =
                    variant_index.expect("`(_, _)` is not a literal").get();
                match algebraic_index {
                    1 => ($(fields.field::<$id>(),)*),
                    _ => panic!("can't instantiate variant #{algebraic_index} of `(_, _)`"),
                }
            }

            #[inline]
            fn deconstruct(self) -> Parts<Store> {
                let mut fields = Store::new();
                // #[expect(clippy::lowercase, reason = "automatically generated")]
                let ($($id,)*) = self;
                $(let () = fields.push($id);)* // TODO: how do we reverse this?
                Parts {
                    fields,
                    variant_index: Some(const { NonZero::new(1).unwrap() }),
                }
            }

            #[inline]
            fn register(registration: &mut Registration<'_>) -> Variants<Self> {
                $(let () = registration.register::<$id>();)*
                let type_ids: [TypeId; _] = [$(TypeId::of::<$id>(),)*];
                Variants::Algebraic(vec![Variant {
                    field_types: type_ids.into_iter().collect(),
                }])
            }
        }
    };
}

impl_for_tuple!();
impl_for_tuple!(A,);
impl_for_tuple!(A, B,);
impl_for_tuple!(A, B, C,);
impl_for_tuple!(A, B, C, D,);
impl_for_tuple!(A, B, C, D, E,);
impl_for_tuple!(A, B, C, D, E, G,);
impl_for_tuple!(A, B, C, D, E, G, H,);
impl_for_tuple!(A, B, C, D, E, G, H, I,);

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "failing tests ought to panic")]

    use {
        crate::{arbitrary::arbitrary, check_eta_expansion, check_serialization},
        pretty_assertions::assert_eq,
        wyrand::WyRand,
    };

    #[test]
    fn deterministic_unit() {
        let mut prng = WyRand::new(42);
        let generated: Vec<()> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<()> = vec![(), (), (), (), (), (), (), (), (), ()];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion_unit() {
        let () = check_eta_expansion::<()>();
    }

    #[test]
    fn serialization_unit() {
        let () = check_serialization::<()>();
    }

    #[test]
    fn deterministic_singleton() {
        let mut prng = WyRand::new(42);
        let generated: Vec<(usize,)> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<(usize,)> = vec![
            (0,),
            (7_804_948_724_862_110_416,),
            (17_108_568_891_541_767_080,),
            (14_756_591_828_928_955_088,),
            (1,),
            (1,),
            (10,),
            (19,),
            (13,),
            (0,),
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion_singleton() {
        let () = check_eta_expansion::<(usize,)>();
    }

    #[test]
    fn serialization_singleton() {
        let () = check_serialization::<(usize,)>();
    }

    #[test]
    fn deterministic_pair() {
        let mut prng = WyRand::new(42);
        let generated: Vec<(usize, bool)> = arbitrary(&mut prng).unwrap().take(10).collect();
        let expected: Vec<(usize, bool)> = vec![
            (1, true),
            (17_728_079_043_341_149_863, false),
            (3_455_211_640_292_790_292, true),
            (0, false),
            (0, false),
            (3, false),
            (6_984_722_224_437_650_403, false),
            (0, false),
            (0, false),
            (1, false),
        ];
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion_pair() {
        let () = check_eta_expansion::<(usize, bool)>();
    }

    #[test]
    fn serialization_pair() {
        let () = check_serialization::<(usize, bool)>();
    }
}
