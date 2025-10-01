//! Corner cases of a type.

/// Corner cases of a type.
pub trait Corner {
    /// Iterator over corner cases of a type.
    type Corners: Iterator<Item = Self>;
    /// Iterate over corner cases of a type.
    fn corners() -> Self::Corners;
}
