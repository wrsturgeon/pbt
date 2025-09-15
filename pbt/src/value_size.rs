use {
    crate::max::{Max, MaybeDecidable, MaybeOverflow},
    core::fmt,
};

pub trait ValueSize: fmt::Debug {
    const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>>;

    fn value_size(&self) -> MaybeOverflow<usize>;
}
