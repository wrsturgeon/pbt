//! Traits that define this library's logic.

pub mod corner;
pub mod decimate;
pub mod refine;
pub mod rnd;
pub mod size;
pub mod weight;

#[macro_export]
macro_rules! impl_tests {
    ($ty:ty, $name:ident) => {
        $crate::traits::weight::impl_weight_tests!($ty, $name);
        $crate::traits::size::impl_size_tests!($ty, $name);
        $crate::traits::decimate::impl_decimate_tests!($ty, $name);
        $crate::traits::refine::impl_refine_tests!($ty, $name);
    };
}
