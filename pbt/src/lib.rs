#![no_std]

//! Property-based testing plus `#[derive(..)]`, no-std, automatic edge cases, and exhaustive breadth-first search over arbitrary types.

pub mod ast_size;
pub mod error;
pub mod exhaust;
mod impls;
pub mod max;
pub mod pseudorandom;
pub mod value_size;

#[macro_export]
macro_rules! test_impls_for {
    ($t:ty, $name:ident $(,)?) => {
        #[cfg(test)]
        mod $name {
            use super::*;

            extern crate alloc;

            const MANY: usize = 1_000;
            const N_SIZES: usize = 10;

            #[test]
            fn max_sizes_agree() {
                let ast_size = <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE;
                let value_size = <$t as $crate::value_size::ValueSize>::MAX_VALUE_SIZE;
                let (ast_size, value_size) = match (&ast_size, &value_size) {
                    (&MaybeDecidable::Decidable(ref ast_size), &MaybeDecidable::Decidable(ref value_size)) => (ast_size, value_size),
                    (&MaybeDecidable::Undecidable, &MaybeDecidable::Undecidable) => return,
                    _ => panic!("Maximum sizes for ASTs and values don't agree: the maximum AST size is {ast_size:?}, but the maximum value size is {value_size:?}"),
                };
                match (ast_size, value_size) {
                    (&$crate::max::Max::Uninstantiable, &$crate::max::Max::Uninstantiable)
                        | (&$crate::max::Max::Finite(_), &$crate::max::Max::Finite(_))
                        | (&$crate::max::Max::Infinite, &$crate::max::Max::Infinite)
                        => {}
                    _ => panic!("Maximum sizes for ASTs and values don't agree: the maximum AST size is {ast_size:?}, but the maximum value size is {value_size:?}"),
                }
            }

            #[test]
            fn max_and_max_expected_sizes_agree() {
                let ast_size = <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE;
                let expected_size = <$t as $crate::ast_size::AstSize>::MAX_EXPECTED_AST_SIZE;
                let (ast_size, expected_size) = match (&ast_size, &expected_size) {
                    (&MaybeDecidable::Decidable(ref ast_size), &MaybeDecidable::Decidable(ref expected_size)) => (ast_size, expected_size),
                    (&MaybeDecidable::Undecidable, &MaybeDecidable::Undecidable) => return,
                    _ => panic!("Maximum AST size and maximum expected AST size don't agree: the maximum AST size is {ast_size:?}, but the maximum expected size is {expected_size:?}"),
                };
                match (ast_size, expected_size) {
                    (&$crate::max::Max::Uninstantiable, &$crate::max::Max::Uninstantiable)
                        | (&$crate::max::Max::Finite(_), &$crate::max::Max::Finite(_))
                        | (&$crate::max::Max::Infinite, &$crate::max::Max::Infinite)
                        => {}
                    _ => panic!("Maximum AST size and maximum expected AST size don't agree: the maximum AST size is {ast_size:?}, but the maximum expected size is {expected_size:?}"),
                }
            }

            #[test]
            fn instantiable_if_claimed() {
                let mut rng = $crate::pseudorandom::default_rng();

                if let MaybeDecidable::Decidable(max) = <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE
                    && max.is_instantiable() && (
                        $crate::exhaust::exhaust::<$t>().next().is_none()
                            || $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).next().is_none()
                    )
                {
                    panic!("Allegedly instantiable type was uninstantiable");
                }
            }

            #[test]
            fn uninstantiable_if_claimed() {
                let mut rng = $crate::pseudorandom::default_rng();

                if matches!(<$t as $crate::ast_size::AstSize>::MAX_AST_SIZE, MaybeDecidable::Decidable($crate::max::Max::Uninstantiable))
                    && let Some(generated) = $crate::exhaust::exhaust::<$t>().next().or_else(|| $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).next())
                {
                    panic!("Allegedly uninstantiable type was instantiated: {generated:#?}");
                }
            }

            #[test]
            fn first_term_has_value_size_zero() {
                let $crate::max::MaybeDecidable::Decidable(max) = <$t as $crate::value_size::ValueSize>::MAX_VALUE_SIZE else {
                    return;
                };
                if matches!(max, $crate::max::Max::Uninstantiable) {
                    return;
                }
                let first: $t = $crate::exhaust::exhaust::<$t>().next().expect("Exhaustive iteration produced no terms!");
                let actual: MaybeOverflow<usize> = <$t as $crate::value_size::ValueSize>::value_size(&first);
                let ideal: MaybeOverflow<usize> = MaybeOverflow::Contained(0);
                assert_eq!(actual, ideal, "The first exhaustively generated term was {first:#?}, but its size was {actual:#?}, not {ideal:#?}");
            }

            #[test]
            fn exhaustive_sizes_are_accurate() {
                for value_size in 0..N_SIZES {
                    let Ok(exhaust) = <$t as $crate::exhaust::Exhaust>::exhaust(value_size) else {
                        return;
                    };
                    let ideal = MaybeOverflow::Contained(value_size);
                    for value in exhaust {
                        let actual = <$t as $crate::value_size::ValueSize>::value_size(&value);
                        assert_eq!(actual, ideal, "Expected a term of value size {ideal:?} but found {value:#?} (of value size {actual:?})");
                    }
                }
            }

            #[test]
            fn no_duplicates_in_exhaustive_search() {
                for value_size in 0..N_SIZES {
                    let Ok(exhaust) = <$t as $crate::exhaust::Exhaust>::exhaust(value_size) else {
                        return;
                    };

                    // Unfortunately, we have to use a `Vec`,
                    // since we can't require `Ord` or `Hash`.
                    let mut seen = alloc::vec::Vec::<$t>::new();

                    for value in exhaust {
                        assert!(!seen.contains(&value), "Duplicate value: {value:#?}");
                        let () = seen.push(value);
                    }
                }
            }

            #[test]
            fn ast_size_always_zero_if_trivial() {
                let mut rng = $crate::pseudorandom::default_rng();

                if let MaybeDecidable::Decidable(max) = <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE
                    && max.is_trivial()
                {
                    for generated in $crate::exhaust::exhaust::<$t>().take(MANY) {
                        assert_eq!(<$t as $crate::ast_size::AstSize>::ast_size(&generated), MaybeOverflow::Contained(0));
                    }
                    for generated in $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).take(MANY) {
                        assert_eq!(<$t as $crate::ast_size::AstSize>::ast_size(&generated), MaybeOverflow::Contained(0));
                    }
                }
            }

            #[test]
            fn max_ast_size_is_accurate() {
                let mut rng = $crate::pseudorandom::default_rng();

                // Exhaust the largest *value* size, if any,
                // checking the *AST* size of each:
                if let MaybeDecidable::Decidable($crate::max::Max::Finite(MaybeOverflow::Contained(max))) = <$t as $crate::value_size::ValueSize>::MAX_VALUE_SIZE
                    && let Ok(exhaust) = <$t as $crate::exhaust::Exhaust>::exhaust(max)
                {
                    for generated in exhaust.take(MANY) {
                        let ast_size = <$t as $crate::ast_size::AstSize>::ast_size(&generated);
                        assert!(
                            ast_size <= MaybeOverflow::Contained(max),
                            "Generated term has an AST size larger than the alleged maximum: {generated:#?} has size {ast_size:?}, but the alleged maximum is {max:?}",
                        );
                    }
                }

                // Pseudorandomly generate the largest AST size, if any:
                if let MaybeDecidable::Decidable($crate::max::Max::Finite(MaybeOverflow::Contained(max))) = <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE {
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_precision_loss,
                        reason = "not meant to be precise"
                    )]
                    let max_f32 = max as f32;
                    for _ in 0..MANY {
                        if let Ok(generated) = <$t as $crate::pseudorandom::Pseudorandom>::pseudorandom(max_f32, &mut rng) {
                            let ast_size = <$t as $crate::ast_size::AstSize>::ast_size(&generated);
                            assert!(
                                ast_size <= MaybeOverflow::Contained(max),
                                "Generated term has an AST size larger than the alleged maximum: {generated:#?} has size {ast_size:?}, but the alleged maximum is {max:?}",
                            );
                        }
                    }
                }
            }

            /*
            #[test]
            fn pseudorandom_exhaustive() {
                const FIRST_N: usize = 16;
                const HAYSTACK_SIZE: usize = MANY.saturating_mul(10);

                let mut size = 0;
                let mut seen: alloc::vec::Vec<($t, bool)> = alloc::vec![];
                'seen: loop {
                    let Ok(iter) = <$t as $crate::exhaust::Exhaust>::exhaust(size) else {
                        break 'seen;
                    };
                    for key in iter {
                        let () = seen.push((key, false));
                        if seen.len() == FIRST_N {
                            break 'seen
                        }
                    }
                    let Some(new_size) = size.checked_add(1) else {
                        break 'seen;
                    };
                    size = new_size;
                }

                #[expect(
                    clippy::as_conversions,
                    clippy::cast_precision_loss,
                    reason = "not meant to be precise"
                )]
                let size = size as f32;

                let mut rng = $crate::pseudorandom::default_rng();
                for _ in 0..HAYSTACK_SIZE {
                    if let Ok(generated) = <$t as $crate::pseudorandom::Pseudorandom>::pseudorandom(size, &mut rng) {
                        'seen: for &mut (ref key, ref mut value) in seen.iter_mut() {
                            if *key == generated {
                                *value = true;
                                if seen.iter().all(|&(_key, value)| value) {
                                    return;
                                }
                                break 'seen;
                            }
                        }
                    }
                }

                let unseen: alloc::vec::Vec<$t> = seen.into_iter().filter_map(|(key, value)| (!value).then_some(key)).collect();
                panic!("The following terms were not produced by `pseudorandom`: {unseen:#?}");
            }
            */

            #[test]
            fn pseudorandom_expected_ast_size_is_accurate() {
                const SIZES: &[usize] = &[1, 10, 100, 1_000];
                const TOLERANCE: f32 = 0.01;

                let MaybeDecidable::Decidable(max_expected) = <$t as $crate::ast_size::AstSize>::MAX_EXPECTED_AST_SIZE else {
                    return;
                };
                if matches!(max_expected, $crate::max::Max::Uninstantiable) {
                    return;
                }

                for &size in SIZES {
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_precision_loss,
                        reason = "not meant to be precise"
                    )]
                    let size = size as f32;
                    if let $crate::max::Max::Finite(max_expected) = max_expected && size > max_expected {
                        return;
                    }

                    let mut rng = $crate::pseudorandom::default_rng();

                    let mut acc = MaybeOverflow::Contained(0_usize);
                    for _ in 0..MANY {
                        if let Ok(generated) = <$t as $crate::pseudorandom::Pseudorandom>::pseudorandom(size, &mut rng) {
                            let ast_size = <$t as $crate::ast_size::AstSize>::ast_size(&generated);
                            acc = acc.plus_self(ast_size);
                        }
                    }

                    if let MaybeOverflow::Contained(acc) = acc {
                        #[expect(
                            clippy::as_conversions,
                            clippy::cast_precision_loss,
                            reason = "not meant to be precise"
                        )]
                        let mean = acc as f32 * const { 1. / (MANY as f32) };
                        let error_absolute = mean - size;
                        let error_relative = error_absolute / size;
                        assert!(
                            ((-TOLERANCE)..=TOLERANCE).contains(&error_relative),
                            "Pseudorandom expected AST size miscalibrated: expected size {size} but found {mean} ({}% relative error)",
                            error_relative * 100.,
                        );
                    }
                }
            }
        }
    };
}
