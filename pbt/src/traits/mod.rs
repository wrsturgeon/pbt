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
        #[cfg(test)]
        mod $name {
            use super::*;
            $crate::impl_weight_tests!($ty, $name);
            $crate::impl_size_tests!($ty, $name);
            $crate::impl_decimate_tests!($ty, $name);
            $crate::impl_refine_tests!($ty, $name);
        }
    };
}
