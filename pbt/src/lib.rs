#![no_std]

//! Property-based testing plus `#[derive(..)]`, no-std, automatic edge cases, and exhaustive breadth-first search over arbitrary types.

pub mod ast_size;
pub mod error;
mod impls;
pub mod pseudorandom;

#[macro_export]
macro_rules! test_impls_for {
    ($t:ty, $name:ident $(,)?) => {
        #[cfg(test)]
        mod $name {
            const MANY: usize = 1_000;

            #[test]
            fn uninstantiable_if_claimed() {
                let mut rng = $crate::pseudorandom::default_rng();

                if matches!(<$t as $crate::ast_size::AstSize>::MAX_AST_SIZE, Ok($crate::ast_size::Max::Uninstantiable))
                    && let Some(generated) = $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).next()
                {
                    panic!("Allegedly uninstantiable type was instantiated: {generated:#?}");
                }
            }

            #[test]
            fn instantiable_if_claimed() {
                let mut rng = $crate::pseudorandom::default_rng();

                if <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE.is_ok_and(|max| max.is_instantiable())
                    && $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).next().is_none()
                {
                    panic!("Allegedly instantiable type was uninstantiable");
                }
            }

            #[test]
            fn ast_size_always_zero_if_trivial() {
                let mut rng = $crate::pseudorandom::default_rng();

                if <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE.is_ok_and(|max| max.is_trivial()) {
                    for generated in $crate::pseudorandom::pseudorandom::<$t, _>(&mut rng).take(MANY) {
                        assert_eq!(<$t as $crate::ast_size::AstSize>::ast_size(&generated), 0);
                    }
                }
            }

            #[test]
            fn max_ast_size_is_accurate() {
                let mut rng = $crate::pseudorandom::default_rng();

                if let Ok($crate::ast_size::Max::Finite(max)) =
                    <$t as $crate::ast_size::AstSize>::MAX_AST_SIZE
                {
                    #[expect(
                        clippy::as_conversions,
                        clippy::cast_precision_loss,
                        reason = "not meant to be precise"
                    )]
                    let max_f32 = max as f32;
                    for _ in 0..MANY {
                        if let Ok(generated) =
                            <$t as $crate::pseudorandom::Pseudorandom>::pseudorandom(max_f32, &mut rng) {
                            let ast_size = <$t as $crate::ast_size::AstSize>::ast_size(&generated);
                            assert!(ast_size <= max, "Generated term has an AST size larger than the alleged maximum: {generated:#?} has size {ast_size:?}, but the alleged maximum is {max:?}");
                        }
                    }
                }
            }
        }
    };
}
