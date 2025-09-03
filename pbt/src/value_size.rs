use crate::max::{Max, MaybeDecidable, MaybeOverflow};

pub trait ValueSize {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>>;

    fn value_size(&self) -> MaybeOverflow<usize>;
}
