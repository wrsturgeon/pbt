use {
    crate::max::{Max, MaybeDecidable, MaybeOverflow},
    core::fmt,
};

pub trait AstSize: fmt::Debug {
    const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>>;
    const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>>;

    fn ast_size(&self) -> MaybeOverflow<usize>;
}
