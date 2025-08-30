use crate::{error, max::Max};

pub trait ValueSize {
    const MAX_VALUE_SIZE: Result<Max<Result<usize, error::Overflow>>, error::Undecidable>;

    fn value_size(&self) -> usize;
}
