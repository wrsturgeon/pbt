//! Proc-macros for `pbt`.

use proc_macro::TokenStream;

/// Derive `::pbt::Pbt` for an arbitrary type.
#[inline]
#[proc_macro_derive(Pbt)]
pub fn derive_pbt(ts: TokenStream) -> TokenStream {
    pbt_macro2::derive_pbt(ts.into()).into()
}

/// Turn a function into a test by throwing inputs at it until it panics.
#[inline]
#[proc_macro_attribute]
pub fn pbt(args: TokenStream, item: TokenStream) -> TokenStream {
    pbt_macro2::pbt(item.into(), args.into()).into()
}
