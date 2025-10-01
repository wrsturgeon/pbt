//! Property-based testing with `#[derive(..)]`, no-std, and automatic edge cases over arbitrary types.

/*
/// The maximum size of a tuple for which we implement all relevant traits.
/// `core::fmt::Debug` is implemented only up to 12, so that's a reasonable choice.
const MAX_TUPLE_SIZE: usize = 12;
*/

/// Implement traits for non-empty tuples of arbitrary (generic) types.
#[proc_macro]
pub fn impl_non_empty_tuples(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    #![expect(clippy::todo, reason = "TODO")]

    let _ts = proc_macro2::TokenStream::new();
    todo!()
}
