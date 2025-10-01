//! Remove large chunks of a value at a time.

/// Test compliance with the crucial invariants assumed of `pbt::Decimate`.
#[macro_export]
macro_rules! impl_decimate_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn decimate() {
            for corner in <$ty as $crate::traits::corner::Corner>::corners() {
                let orig_weight = <$ty as $crate::traits::weight::Weight>::weight(&corner);
                for expected_weight in 0..orig_weight {
                    for decimated in <$ty as $crate::traits::decimate::Decimate>::decimate(&corner, expected_weight) {
                        let actual_weight = <$ty as $crate::traits::weight::Weight>::weight(&decimated);
                        assert_eq!(actual_weight, expected_weight, "Decimating `{corner:#?}` to weight {expected_weight} produced a value, `{decimated:#?}`, of weight {actual_weight} (=/= {expected_weight})");
                    }
                }
                let mut decimate = <$ty as $crate::traits::decimate::Decimate>::decimate(&corner, orig_weight);
                let Some(decimated) = decimate.next() else {
                    panic!("Decimating `{corner:#?}` to its own weight ({orig_weight}) produced no values!");
                };
                assert_eq!(decimated, corner, "Decimating `{corner:#?}` to its own weight ({orig_weight}) produced `{decimated:#?}`, but it should have produced only itself!");
                if let Some(decimated) = decimate.next() {
                    panic!("Decimating `{corner:#?}` to its own weight ({orig_weight}) produced itself and then another value, `{decimated:#?}`, instead of itself alone!");
                }
            }
        }
    };
}

/// Remove large chunks of a value at a time.
/// # Technical definition
/// Technically, this decreases the `Weight` of a value
/// without touching zero-`Size` values.
pub trait Decimate {
    /// Iterator over decimated values.
    type Decimate: Iterator<Item = Self>;
    /// Remove large chunks of this value at a time
    /// to produce smaller values of the requested weight.
    fn decimate(&self, weight: usize) -> Self::Decimate;
}
