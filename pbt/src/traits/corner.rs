//! Corner cases of a type.

/// Corner cases of a type.
pub trait Corner {
    type Corners: Iterator<Item = Self>;
    fn corners() -> Self::Corners;
}
