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

/// Push tuple fields into a `Store` from right to left.
macro_rules! push_tuple_fields_reversed {
    ($fields:ident;) => {};

    ($fields:ident; $head:ident, $($tail:ident,)*) => {
        push_tuple_fields_reversed!($fields; $($tail,)*);
        let () = $fields.push($head);
    };
}

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
                push_tuple_fields_reversed!(fields; $($id,)*);
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
        crate::{
            arbitrary::arbitrary, check_eta_expansion, check_serialization, persist,
            reflection::register_globally,
        },
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
        let () = register_globally::<(usize,)>();
        let mut prng = WyRand::new(42);
        let mut expected: Vec<(usize,)> = persist::replay();
        let () = expected.extend([
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
        ]);
        let generated: Vec<(usize,)> = arbitrary(&mut prng).unwrap().take(expected.len()).collect();
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
        let () = register_globally::<(usize, bool)>();
        let mut prng = WyRand::new(42);
        let mut expected: Vec<(usize, bool)> = persist::replay();
        let () = expected.extend([
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
        ]);
        let generated: Vec<(usize, bool)> =
            arbitrary(&mut prng).unwrap().take(expected.len()).collect();
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

    #[test]
    fn deterministic_triple() {
        let () = register_globally::<(usize, bool, bool)>();
        let mut prng = WyRand::new(42);
        let mut expected: Vec<(usize, bool, bool)> = persist::replay();
        let () = expected.extend([
            (1, true, false),
            (13_639_797_723_846_260_844, false, true),
            (367_415_042_230_254_170, false, true),
            (14_075_417_872_264_614_812, false, false),
            (15_963_154_638_716_436_219, false, false),
            (5_536_629_187_452_512_295, false, false),
            (0, false, false),
            (0, false, false),
            (0, false, false),
            (11, false, false),
        ]);
        let generated: Vec<(usize, bool, bool)> =
            arbitrary(&mut prng).unwrap().take(expected.len()).collect();
        assert_eq!(generated, expected);
    }

    #[test]
    fn eta_expansion_triple() {
        let () = check_eta_expansion::<(usize, bool, bool)>();
    }

    #[test]
    fn serialization_triple() {
        let () = check_serialization::<(usize, bool, bool)>();
    }
}
