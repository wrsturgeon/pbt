//! Proc-macros for `pbt`.

use proc_macro::TokenStream;

/// Derive `::pbt::Pbt` for an arbitrary type.
#[inline]
#[proc_macro_derive(Pbt)]
pub fn derive_pbt(ts: TokenStream) -> TokenStream {
    pbt_macro2::derive_pbt(ts.into()).into()
}
