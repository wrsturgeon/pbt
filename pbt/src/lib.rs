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
            const MANY: usize = 1_000;

            #[test]
            fn max_sizes_agree() {
                let ast_size = <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE;
                let value_size = <$t as $crate::value_size::ValueSize>::MAX_VALUE_SIZE;
                let (ast_size, value_size) = match (&ast_size, &value_size) {
                    (&Ok(ref ast_size), &Ok(ref value_size)) => (ast_size, value_size),
                    (&Err($crate::error::Undecidable), &Err($crate::error::Undecidable)) => return,
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
            fn instantiable_if_claimed() {
                let mut rng = $crate::pseudorandom::default_rng();

                if <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE.is_ok_and(|max| max.is_instantiable())
                    && ($crate::exhaust::exhaust::<$t>().next().is_none()
                        || $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).next().is_none())
                {
                    panic!("Allegedly instantiable type was uninstantiable");
                }
            }

            #[test]
            fn uninstantiable_if_claimed() {
                let mut rng = $crate::pseudorandom::default_rng();

                if matches!(<$t as $crate::ast_size::AstSize>::MAX_AST_SIZE, Ok($crate::max::Max::Uninstantiable))
                    && let Some(generated) = $crate::exhaust::exhaust::<$t>().next().or_else(|| $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).next())
                {
                    panic!("Allegedly uninstantiable type was instantiated: {generated:#?}");
                }
            }

            #[test]
            fn ast_size_always_zero_if_trivial() {
                let mut rng = $crate::pseudorandom::default_rng();

                if <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE.is_ok_and(|max| max.is_trivial()) {
                    for generated in $crate::exhaust::exhaust::<$t>().take(MANY) {
                        assert_eq!(<$t as $crate::ast_size::AstSize>::ast_size(&generated), 0);
                    }
                    for generated in $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).take(MANY) {
                        assert_eq!(<$t as $crate::ast_size::AstSize>::ast_size(&generated), 0);
                    }
                }
            }

            #[test]
            fn max_ast_size_is_accurate() {
                let mut rng = $crate::pseudorandom::default_rng();

                // Exhaust the largest *value* size, if any,
                // checking the *AST* size of each:
                if let Ok($crate::max::Max::Finite(max)) = <$t as $crate::value_size::ValueSize>::MAX_VALUE_SIZE
                    && let max = max.unwrap_or(usize::MAX)
                    && let Ok(exhaust) = <$t as $crate::exhaust::Exhaust>::exhaust(max)
                {
                    for generated in exhaust.take(MANY) {
                        let ast_size = <$t as $crate::ast_size::AstSize>::ast_size(&generated);
                        assert!(ast_size <= max, "Generated term has an AST size larger than the alleged maximum: {generated:#?} has size {ast_size:?}, but the alleged maximum is {max:?}");
                    }
                }

                // Pseudorandomly generate the largest AST size, if any:
                if let Ok($crate::max::Max::Finite(Ok(max))) = <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE {
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_precision_loss,
                        reason = "not meant to be precise"
                    )]
                    let max_f32 = max as f32;
                    for _ in 0..MANY {
                        if let Ok(generated) = <$t as $crate::pseudorandom::Pseudorandom>::pseudorandom(max_f32, &mut rng) {
                            let ast_size = <$t as $crate::ast_size::AstSize>::ast_size(&generated);
                            assert!(ast_size <= max, "Generated term has an AST size larger than the alleged maximum: {generated:#?} has size {ast_size:?}, but the alleged maximum is {max:?}");
                        }
                    }
                }
            }
        }
    };
}
