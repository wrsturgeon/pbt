//! Proc-macros for `pbt`, using the `proc-macro2` crate for reusability.

use {proc_macro2::TokenStream, syn::DeriveInput};

/// Derive `::pbt::Pbt` for an arbitrary type.
#[inline]
pub fn derive_pbt(ts: TokenStream) -> TokenStream {
    try_derive_pbt(ts).unwrap_or_else(syn::Error::into_compile_error)
}

/// Derive `::pbt::Pbt` for an arbitrary type.
///
/// # Errors
///
/// If the input type is not up to the task.
#[inline]
pub fn try_derive_pbt(ts: TokenStream) -> syn::Result<TokenStream> {
    let DeriveInput {
        data: input_data,
        ident,
        ..
    } = syn::parse2(ts)?;
    let syn::Data::Enum(enum_data) = input_data else {
        return Err(syn::Error::new_spanned(
            ident,
            "`Pbt` can currently be derived only for enums",
        ));
    };

    let literal_message = format!("`{ident}` is not a literal");
    let bad_variant_message =
        format!("can't instantiate variant #{{algebraic_index}} of `{ident}`");

    let mut construct_arms = Vec::new();
    let mut deconstruct_arms = Vec::new();
    let mut register_pushes = Vec::new();
    for (zero_index, variant) in enum_data.variants.iter().enumerate() {
        let syn::Fields::Unit = variant.fields else {
            return Err(syn::Error::new_spanned(
                variant,
                "`Pbt` can currently be derived only for fieldless variants",
            ));
        };
        let one_index = zero_index
            .checked_add(1)
            .ok_or_else(|| syn::Error::new_spanned(variant, "too many variants"))?;
        let one_index_string = one_index.to_string();
        let construct_index = syn::LitInt::new(&one_index_string, variant.ident.span());
        let deconstruct_index = syn::LitInt::new(&one_index_string, variant.ident.span());
        let variant_ident = &variant.ident;
        construct_arms.push(quote::quote! {
            #construct_index => Self::#variant_ident
        });
        deconstruct_arms.push(quote::quote! {
            Self::#variant_ident => ::pbt::reflection::Parts {
                fields: {
                    let mut acc = ::pbt::fields::Store::new();
                    acc
                },
                variant_index: Some(const { ::core::num::NonZero::new(#deconstruct_index).unwrap() }),
            }
        });
        register_pushes.push(quote::quote! {
            let () = acc.push(::pbt::reflection::Variant {
                field_types: ::pbt::multiset::Multiset::new(),
            });
        });
    }

    Ok(quote::quote! {
        impl ::pbt::Pbt for #ident {
            #[inline]
            fn construct<F>(::pbt::reflection::Parts { fields, variant_index }: ::pbt::reflection::Parts<F>) -> Self
            where
                F: ::pbt::fields::Fields,
            {
                let algebraic_index: usize = variant_index.expect(#literal_message).get();
                match algebraic_index {
                    #(#construct_arms,)*
                    _ => panic!(#bad_variant_message),
                }
            }

            #[inline]
            fn deconstruct(self) -> ::pbt::reflection::Parts<::pbt::fields::Store> {
                match self {
                    #(#deconstruct_arms,)*
                }
            }

            #[inline]
            fn register(registration: &mut ::pbt::registration::Registration<'_>) -> ::pbt::reflection::Variants<Self> {
                let mut acc = vec![];
                #(#register_pushes)*
                ::pbt::reflection::Variants::Algebraic(acc)
            }
        }
    })
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "Failing tests ought to panic.")]
    #![expect(clippy::needless_raw_strings, reason = "Consistency.")]

    use {super::*, pretty_assertions::assert_eq};

    #[inline]
    fn expect_test(input: &str, output: &str) {
        let unformatted = syn::parse2(derive_pbt(input.parse().expect("input couldn't be parsed")))
            .expect("derived output couldn't be parsed");
        let formatted = prettyplease::unparse(&unformatted);
        let actual = formatted.trim();
        let expected = output.trim();
        assert_eq!(actual, expected);
    }

    #[test]
    fn bool() {
        expect_test(
            r#"
enum Bool {
    False,
    True,
}
"#,
            r#"
impl ::pbt::Pbt for Bool {
    #[inline]
    fn construct<F>(
        ::pbt::reflection::Parts { fields, variant_index }: ::pbt::reflection::Parts<F>,
    ) -> Self
    where
        F: ::pbt::fields::Fields,
    {
        let algebraic_index: usize = variant_index
            .expect("`Bool` is not a literal")
            .get();
        match algebraic_index {
            1 => Self::False,
            2 => Self::True,
            _ => panic!("can't instantiate variant #{algebraic_index} of `Bool`"),
        }
    }
    #[inline]
    fn deconstruct(self) -> ::pbt::reflection::Parts<::pbt::fields::Store> {
        match self {
            Self::False => {
                ::pbt::reflection::Parts {
                    fields: {
                        let mut acc = ::pbt::fields::Store::new();
                        acc
                    },
                    variant_index: Some(const { ::core::num::NonZero::new(1).unwrap() }),
                }
            }
            Self::True => {
                ::pbt::reflection::Parts {
                    fields: {
                        let mut acc = ::pbt::fields::Store::new();
                        acc
                    },
                    variant_index: Some(const { ::core::num::NonZero::new(2).unwrap() }),
                }
            }
        }
    }
    #[inline]
    fn register(
        registration: &mut ::pbt::registration::Registration<'_>,
    ) -> ::pbt::reflection::Variants<Self> {
        let mut acc = vec![];
        let () = acc
            .push(::pbt::reflection::Variant {
                field_types: ::pbt::multiset::Multiset::new(),
            });
        let () = acc
            .push(::pbt::reflection::Variant {
                field_types: ::pbt::multiset::Multiset::new(),
            });
        ::pbt::reflection::Variants::Algebraic(acc)
    }
}
"#,
        );
    }
}
