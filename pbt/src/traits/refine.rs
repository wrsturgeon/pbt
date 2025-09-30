//! Remove small details of a value.

#[macro_export]
macro_rules! impl_refine_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn refine() {
            for corner in <$ty as $crate::traits::corner::Corner>::corners() {
                let orig_weight = <$ty as $crate::traits::weight::Weight>::weight(&corner);
                let orig_size = <$ty as $crate::traits::size::Size>::size(&corner);
                let mut refine = <$ty as $crate::traits::refine::Refine>::refine(&corner);
                while let Some(refined) = refine.next() {
                    let weight = <$ty as $crate::traits::weight::Weight>::weight(&refined);
                    assert_eq!(
                        weight,
                        orig_weight,
                        "Refinement produced a value of a different weight than the original! Weight of the refined value (`{refined:#?}`) was {weight:?}, but the weight of the original (`{corner:#?}`) was {orig_weight:?}",
                    );
                    let size = <$ty as $crate::traits::size::Size>::size(&refined);
                    match PartialOrd::partial_cmp(&size, &orig_size) {
                        None | Some(core::cmp::Ordering::Less) => continue,
                        Some(core::cmp::Ordering::Greater) => panic!(
                            "Refinement produced a value larger than the original! Size of the refined value (`{refined:#?}`) was {size:?}, but the size of the original (`{corner:#?}`) was only {orig_size:?}",
                        ),
                        Some(core::cmp::Ordering::Equal) => {
                            assert_eq!(
                                refined,
                                corner,
                                "Refinement produced a value (`{refined:#?}`) of size ({size:?}) equal to that of the original (`{corner:#?}`), but the two values were not equal! (The only value of equal size should be the original value at the end.)",
                            );
                            return;
                        }
                    }
                }
                panic!("Refinement never produced the original input (`{corner:#?}`)!");
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
    fn refine(&self) -> Self::Refine;
}
