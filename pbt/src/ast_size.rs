use crate::{error, max::Max};

pub trait AstSize {
    const MAX_AST_SIZE: Result<Max<Result<usize, error::Overflow>>, error::Undecidable>;

    fn ast_size(&self) -> usize;
}
