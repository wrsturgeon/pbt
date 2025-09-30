//! Remove large chunks of a value at a time.

#[macro_export]
macro_rules! impl_decimate_tests {
    ($ty:ty, $name:ident) => {
        #[test]
        fn decimate() {
            for corner in <$ty as $crate::traits::corner::Corner>::corners() {
                let orig_weight = <$ty as $crate::traits::weight::Weight>::weight(&corner);
                let mut decimate = <$ty as $crate::traits::decimate::Decimate>::decimate(&corner);
                while let Some(decimated) = decimate.next() {
                    let weight = <$ty as $crate::traits::weight::Weight>::weight(&decimated);
                    match PartialOrd::partial_cmp(&weight, &orig_weight) {
                        None | Some(core::cmp::Ordering::Less) => continue,
                        Some(core::cmp::Ordering::Greater) => panic!(
                            "Decimation produced a value heavier than the original! Weight of the decimated value (`{decimated:#?}`) was {weight:?}, but the weight of the original (`{corner:#?}`) was only {orig_weight:?}",
                        ),
                        Some(core::cmp::Ordering::Equal) => {
                            assert_eq!(
                                decimated,
                                corner,
                                "Decimation produced a value (`{decimated:#?}`) of weight ({weight:?}) equal to that of the original (`{corner:#?}`), but the two values were not equal! (The only value of equal weight should be the original value at the end.)",
                            );
                            return;
                        }
                    }
                }
                panic!("Decimation never produced the original input (`{corner:#?}`)!");
            }
        }
    };
}

/// Remove large chunks of a value at a time.
/// # Technical definition
/// Technically, this decreases the `Weight` of a value
/// without touching zero-`Size` values.
pub trait Decimate {
    type Decimate: Iterator<Item = Self>;
    fn decimate(&self) -> Self::Decimate;
}
