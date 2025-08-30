//! Implementations for `bool`.

use crate::{
    ast_size::{AstSize, Max},
    error,
    pseudorandom::Pseudorandom,
    test_impls_for,
};

impl AstSize for bool {
    const MAX_AST_SIZE: Result<Max<usize>, error::Undecidable> = Ok(Max::Finite(0));

    #[inline]
    fn ast_size(&self) -> usize {
        0
    }
}
impl Pseudorandom for bool {
    #[inline]
    fn pseudorandom<Rng: rand_core::RngCore>(
        _expected_ast_size: f32,
        rng: &mut Rng,
    ) -> Result<Self, error::Uninstantiable> {
        Ok(rng.next_u32() & 1 != 0)
    }
}
test_impls_for!(bool, bool);
