//! Remove small details of a value.

#[macro_export]
macro_rules! impl_refine_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn refine() {
            'corners: for corner in <$ty as $crate::traits::corner::Corner>::corners() {
                let orig_weight = <$ty as $crate::traits::weight::Weight>::weight(&corner);
                let orig_size = <$ty as $crate::traits::size::Size>::size(&corner);
                let $crate::size::MaybeOverflow::Contained(orig_size) = orig_size else {
                    continue 'corners;
                };
                for expected_size in 0..orig_size {
                    for refined in <$ty as $crate::traits::refine::Refine>::refine(&corner, expected_size) {
                        let actual_weight = <$ty as $crate::traits::weight::Weight>::weight(&refined);
                        assert_eq!(orig_weight, actual_weight, "Refining `{corner:#?}` to weight {orig_weight} produced a value, `{refined:#?}`, of weight {actual_weight} (=/= {orig_weight})");
                        let actual_size = <$ty as $crate::traits::size::Size>::size(&refined);
                        assert_eq!(
                            actual_size,
                            $crate::size::MaybeOverflow::Contained(expected_size),
                            "Refining `{corner:#?}` to size {expected_size} produced a value, `{refined:#?}`, of size {actual_size:?} (=/= {expected_size})",
                        );
                    }
                }
                let mut refine = <$ty as $crate::traits::refine::Refine>::refine(&corner, orig_size);
                let Some(refined) = refine.next() else {
                    panic!("Refining `{corner:#?}` to its own size ({orig_size}) produced no values!");
                };
                assert_eq!(refined, corner, "Refining `{corner:#?}` to its own size ({orig_size}) produced `{refined:#?}`, but it should have produced only itself!");
                if let Some(refined) = refine.next() {
                    panic!("Refining `{corner:#?}` to its own size ({orig_size}) produced itself and then another value, `{refined:#?}`, instead of itself alone!");
                }
            }
        }
    };
}

/// Remove small details of a value.
/// # Technical definition
/// Technically, this manitains the `Weight` of a value
/// while decreasing its `Size` alone.
pub trait Refine {
    type Refine: Iterator<Item = Self>;
    fn refine(&self, size: usize) -> Self::Refine;
}
