extern crate alloc;

use {alloc::vec::Vec, pbt::edge_cases::EdgeCases};

type T = (u8, u8, u8, u8);

fn main() {
    let edge_cases = <T as EdgeCases>::edge_cases();

    // Unfortunately, we have to use a `Vec`,
    // since we can't require `Ord` or `Hash`.
    let mut seen = Vec::<T>::new();

    for value in edge_cases {
        // println!("{value:?}");
        assert!(
            !seen.contains(&value),
            "Duplicate value (seen while generating edge cases): {value:#?} (list of seen before it: {seen:#?})"
        );
        let () = seen.push(value);
    }
}
