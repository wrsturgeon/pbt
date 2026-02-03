#![allow(
    clippy::missing_inline_in_public_items,
    reason = "macros are the only public items"
)]

use proc_macro::TokenStream;

#[proc_macro_derive(Pbt)]
pub fn derive_pbt(ts: TokenStream) -> TokenStream {
    todo!("{ts:#?}")
}
