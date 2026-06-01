//! Proc-macros for `pbt`, using the `proc-macro2` crate for reusability.

use {
    proc_macro2::TokenStream,
    quote::quote,
    syn::{
        Data, DeriveInput, Expr, ExprLit, Fields, FnArg, ItemFn, Lit, LitInt, Pat, ReturnType, Type,
    },
};

/// Derive `::pbt::Pbt` for an arbitrary type.
#[inline]
pub fn derive_pbt(ts: TokenStream) -> TokenStream {
    try_derive_pbt(ts).unwrap_or_else(syn::Error::into_compile_error)
}

/// Turn a function into a test by throwing inputs at it until it panics.
#[inline]
pub fn pbt(item: TokenStream, args: TokenStream) -> TokenStream {
    try_pbt(item, args).unwrap_or_else(syn::Error::into_compile_error)
}

/// Derive `::pbt::Pbt` for an arbitrary type.
///
/// # Errors
///
/// If the input is not up to the task.
#[inline]
pub fn try_derive_pbt(ts: TokenStream) -> syn::Result<TokenStream> {
    struct Pattern {
        construction: TokenStream,
        deconstruction: TokenStream,
        field_pushes: Vec<TokenStream>,
        field_type_inserts: Vec<TokenStream>,
        span: proc_macro2::Span,
    }

    fn pattern(
        head: TokenStream,
        fields: &Fields,
        span: proc_macro2::Span,
    ) -> syn::Result<Pattern> {
        match *fields {
            Fields::Unit => Ok(Pattern {
                construction: head.clone(),
                deconstruction: head,
                field_pushes: Vec::new(),
                field_type_inserts: Vec::new(),
                span,
            }),
            Fields::Unnamed(ref unnamed_fields) => {
                let mut field_bindings = Vec::new();
                let mut field_constructions = Vec::new();
                let mut field_pushes = Vec::new();
                let mut field_type_inserts = Vec::new();
                for (index, field) in unnamed_fields.unnamed.iter().enumerate() {
                    let field_binding = quote::format_ident!("_anonymous_{index}");
                    field_constructions.push(quote! { fields.field() });
                    let ty = &field.ty;
                    field_type_inserts.push(quote! {
                        let () = registration.register::<#ty>();
                        let () = acc.insert(::core::any::TypeId::of::<#ty>());
                    });
                    field_bindings.push(field_binding);
                }
                for field_binding in field_bindings.iter().rev() {
                    field_pushes.push(quote! {
                        let () = acc.push(#field_binding);
                    });
                }
                Ok(Pattern {
                    construction: quote! { #head(#(#field_constructions),*) },
                    deconstruction: quote! { #head(#(#field_bindings),*) },
                    field_pushes,
                    field_type_inserts,
                    span,
                })
            }
            Fields::Named(ref named_fields) => {
                let mut field_bindings = Vec::new();
                let mut field_pushes = Vec::new();
                let mut field_type_inserts = Vec::new();
                for field in &named_fields.named {
                    let Some(field_binding) = field.ident.clone() else {
                        return Err(syn::Error::new_spanned(field, "missing field name"));
                    };
                    let ty = &field.ty;
                    field_type_inserts.push(quote! {
                        let () = registration.register::<#ty>();
                        let () = acc.insert(::core::any::TypeId::of::<#ty>());
                    });
                    field_bindings.push(field_binding);
                }
                for field_binding in field_bindings.iter().rev() {
                    field_pushes.push(quote! {
                        let () = acc.push(#field_binding);
                    });
                }
                Ok(Pattern {
                    construction: quote! { #head { #(#field_bindings: fields.field()),* } },
                    deconstruction: quote! { #head { #(#field_bindings),* } },
                    field_pushes,
                    field_type_inserts,
                    span,
                })
            }
        }
    }

    let DeriveInput {
        data: input_data,
        generics,
        ident,
        ..
    } = syn::parse2(ts)?;
    let patterns = match input_data {
        Data::Enum(enum_data) => enum_data
            .variants
            .iter()
            .map(|variant| {
                let variant_ident = &variant.ident;
                pattern(
                    quote! { Self::#variant_ident },
                    &variant.fields,
                    variant.ident.span(),
                )
            })
            .collect::<syn::Result<Vec<_>>>()?,
        Data::Struct(struct_data) => {
            vec![pattern(quote! { Self }, &struct_data.fields, ident.span())?]
        }
        Data::Union(_) => {
            return Err(syn::Error::new_spanned(
                ident,
                "`Pbt` can currently be derived only for structs and enums",
            ));
        }
    };

    let literal_message = format!("`{ident}` is not a literal");
    let bad_variant_message =
        format!("can't instantiate variant #{{algebraic_index}} of `{ident}`");
    let mut bounded_generics = generics;
    for parameter in bounded_generics.type_params_mut() {
        parameter.bounds.push(syn::parse_quote! { ::pbt::Pbt });
    }
    let (impl_generics, ty_generics, where_clause) = bounded_generics.split_for_impl();

    let mut construct_arms = Vec::new();
    let mut deconstruct_arms = Vec::new();
    let mut register_pushes = Vec::new();
    for (zero_index, pattern) in patterns.iter().enumerate() {
        let one_index = zero_index
            .checked_add(1)
            .ok_or_else(|| syn::Error::new_spanned(&ident, "too many patterns"))?;
        let one_index_string = one_index.to_string();
        let construct_index = LitInt::new(&one_index_string, pattern.span);
        let deconstruct_index = LitInt::new(&one_index_string, pattern.span);
        let construction = &pattern.construction;
        let deconstruction = &pattern.deconstruction;
        let field_pushes = &pattern.field_pushes;
        let field_type_inserts = &pattern.field_type_inserts;
        construct_arms.push(quote! {
            #construct_index => #construction
        });
        deconstruct_arms.push(quote! {
            #deconstruction => ::pbt::reflection::Parts {
                fields: {
                    let mut acc = ::pbt::fields::Store::new();
                    #(#field_pushes)*
                    acc
                },
                variant_index: Some(const { ::core::num::NonZero::new(#deconstruct_index).unwrap() }),
            }
        });
        register_pushes.push(quote! {
            let () = acc.push(::pbt::reflection::Variant {
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    #(#field_type_inserts)*
                    acc
                },
            });
        });
    }

    Ok(quote! {
        impl #impl_generics ::pbt::Pbt for #ident #ty_generics #where_clause {
            #[inline]
            fn construct<F>(
                ::pbt::reflection::Parts {
                    mut fields,
                    variant_index,
                }: ::pbt::reflection::Parts<F>,
            ) -> Self
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

/// Turn a function into a test by throwing inputs at it until it panics.
///
/// # Errors
///
/// If the input is not up to the task.
#[inline]
pub fn try_pbt_with_cases(ts: TokenStream, n_cases: Option<usize>) -> syn::Result<TokenStream> {
    let n_cases_expr = if let Some(n) = n_cases {
        let digits = n.to_string();
        let mut grouped = String::new();
        for (index, ch) in digits.chars().enumerate() {
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "A character index from `enumerate` cannot exceed the string's length."
            )]
            if index != 0 && (digits.len() - index).is_multiple_of(3) {
                grouped.push('_');
            }
            grouped.push(ch);
        }
        Expr::Lit(ExprLit {
            attrs: vec![],
            lit: Lit::Int(LitInt::new(&grouped, proc_macro2::Span::call_site())),
        })
    } else {
        Expr::Verbatim(quote! { if cfg!(miri) { 100 } else { 10_000 } })
    };
    let ItemFn {
        attrs, block, sig, ..
    } = syn::parse2(ts)?;
    if sig.constness.is_some() {
        return Err(syn::Error::new_spanned(
            sig.constness,
            "`#[pbt]` does not support `const fn`",
        ));
    }
    if sig.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            sig.asyncness,
            "`#[pbt]` does not support `async fn`",
        ));
    }
    if sig.unsafety.is_some() {
        return Err(syn::Error::new_spanned(
            sig.unsafety,
            "`#[pbt]` does not support `unsafe fn`",
        ));
    }
    if sig.abi.is_some() {
        return Err(syn::Error::new_spanned(
            sig.abi,
            "`#[pbt]` does not support explicit ABIs",
        ));
    }
    if !sig.generics.params.is_empty() || sig.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            sig.generics,
            "`#[pbt]` does not support generic functions",
        ));
    }
    if !matches!(sig.output, ReturnType::Default) {
        return Err(syn::Error::new_spanned(
            sig.output,
            "`#[pbt]` test functions must return `()`",
        ));
    }
    let inputs = sig
        .inputs
        .iter()
        .map(|input_arg| {
            let input = match *input_arg {
                FnArg::Typed(ref input) => input,
                FnArg::Receiver(_) => {
                    return Err(syn::Error::new_spanned(
                        input_arg,
                        "`#[pbt]` test functions cannot accept `self`",
                    ));
                }
            };
            if let Some(attribute) = input.attrs.first() {
                return Err(syn::Error::new_spanned(
                    attribute,
                    "`#[pbt]` test function inputs cannot have attributes",
                ));
            }
            Ok(input)
        })
        .collect::<syn::Result<Vec<_>>>()?;
    if inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            sig.ident,
            "`#[pbt]` test functions must accept at least one input",
        ));
    }
    let (pat, ty) = if let Some(input) = inputs.first().filter(|_| inputs.len() == 1) {
        let pat = &input.pat;
        let ty = &input.ty;
        (quote! { #pat }, quote! { #ty })
    } else {
        let mut pats = Vec::new();
        let mut tys = Vec::new();
        for input in inputs {
            let Pat::Ident(ref pat) = *input.pat else {
                return Err(syn::Error::new_spanned(
                    &input.pat,
                    "`#[pbt]` test functions with multiple inputs must use identifier patterns",
                ));
            };
            let Type::Reference(ref reference) = *input.ty else {
                return Err(syn::Error::new_spanned(
                    &input.ty,
                    "`#[pbt]` test function inputs must be shared references",
                ));
            };
            if reference.mutability.is_some() {
                return Err(syn::Error::new_spanned(
                    &input.ty,
                    "`#[pbt]` test function inputs must be shared references",
                ));
            }
            let ident = &pat.ident;
            let ty = &reference.elem;
            pats.push(quote! { ref #ident });
            tys.push(quote! { #ty });
        }
        (quote! { &(#(#pats),*) }, quote! { &(#(#tys),*) })
    };
    let ident = sig.ident;

    Ok(quote! {
        #[test]
        #(#attrs)*
        fn #ident() {
            let mut prng = ::pbt::WyRand::new(42);
            let maybe_witness = pbt::witness(
                |#pat: #ty| -> Option<Option<String>> {
                    ::pbt::panic::catch(move || #block).err()
                },
                #n_cases_expr,
                &mut prng,
            );
            if let Some((witness, maybe_panic_msg)) = maybe_witness {
                if let Some(panic_msg) = maybe_panic_msg {
                    panic!(
                        "\r\nConsider the following input:\r\n\r\n```\r\n{witness:#?}\r\n```\r\n\r\n{panic_msg}",
                    );
                } else {
                    panic!(
                        "\r\nConsider the following input:\r\n\r\n```\r\n{witness:#?}\r\n```\r\n\r\nThis panicked, but the payload was not recoverable.",
                    );
                }
            }
        }
    })
}

/// Turn a function into a test by throwing inputs at it until it panics.
///
/// # Errors
///
/// If the input is not up to the task.
#[inline]
pub fn try_pbt(item: TokenStream, args: TokenStream) -> syn::Result<TokenStream> {
    let n_cases = if args.is_empty() {
        None
    } else {
        Some(syn::parse2::<LitInt>(args)?.base10_parse()?)
    };
    try_pbt_with_cases(item, n_cases)
}

#[cfg(test)]
mod tests {
    #![expect(clippy::expect_used, reason = "Failing tests ought to panic.")]
    #![expect(clippy::needless_raw_strings, reason = "Consistency.")]

    use {super::*, pretty_assertions::assert_eq, quote::ToTokens as _};

    #[inline]
    fn expect_test(input: &str, function: fn(TokenStream) -> TokenStream, output: &str) {
        let unformatted = syn::parse2(function(input.parse().expect("input couldn't be parsed")))
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
            derive_pbt,
            r#"
impl ::pbt::Pbt for Bool {
    #[inline]
    fn construct<F>(
        ::pbt::reflection::Parts {
            mut fields,
            variant_index,
        }: ::pbt::reflection::Parts<F>,
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
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    acc
                },
            });
        let () = acc
            .push(::pbt::reflection::Variant {
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    acc
                },
            });
        ::pbt::reflection::Variants::Algebraic(acc)
    }
}
"#,
        );
    }

    #[test]
    fn unit() {
        expect_test(
            r#"
struct Unit;
"#,
            derive_pbt,
            r#"
impl ::pbt::Pbt for Unit {
    #[inline]
    fn construct<F>(
        ::pbt::reflection::Parts {
            mut fields,
            variant_index,
        }: ::pbt::reflection::Parts<F>,
    ) -> Self
    where
        F: ::pbt::fields::Fields,
    {
        let algebraic_index: usize = variant_index
            .expect("`Unit` is not a literal")
            .get();
        match algebraic_index {
            1 => Self,
            _ => panic!("can't instantiate variant #{algebraic_index} of `Unit`"),
        }
    }
    #[inline]
    fn deconstruct(self) -> ::pbt::reflection::Parts<::pbt::fields::Store> {
        match self {
            Self => {
                ::pbt::reflection::Parts {
                    fields: {
                        let mut acc = ::pbt::fields::Store::new();
                        acc
                    },
                    variant_index: Some(const { ::core::num::NonZero::new(1).unwrap() }),
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
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    acc
                },
            });
        ::pbt::reflection::Variants::Algebraic(acc)
    }
}
"#,
        );
    }

    #[test]
    fn lambda_calculus() {
        expect_test(
            r#"
enum LambdaCalculus {
    Application(Box<Self>, Box<Self>),
    Lambda {
        body: Box<Self>,
    },
    Variable {
        de_bruijn: usize,
    },
}
"#,
            derive_pbt,
            r#"
impl ::pbt::Pbt for LambdaCalculus {
    #[inline]
    fn construct<F>(
        ::pbt::reflection::Parts {
            mut fields,
            variant_index,
        }: ::pbt::reflection::Parts<F>,
    ) -> Self
    where
        F: ::pbt::fields::Fields,
    {
        let algebraic_index: usize = variant_index
            .expect("`LambdaCalculus` is not a literal")
            .get();
        match algebraic_index {
            1 => Self::Application(fields.field(), fields.field()),
            2 => {
                Self::Lambda {
                    body: fields.field(),
                }
            }
            3 => {
                Self::Variable {
                    de_bruijn: fields.field(),
                }
            }
            _ => {
                panic!(
                    "can't instantiate variant #{algebraic_index} of `LambdaCalculus`"
                )
            }
        }
    }
    #[inline]
    fn deconstruct(self) -> ::pbt::reflection::Parts<::pbt::fields::Store> {
        match self {
            Self::Application(_anonymous_0, _anonymous_1) => {
                ::pbt::reflection::Parts {
                    fields: {
                        let mut acc = ::pbt::fields::Store::new();
                        let () = acc.push(_anonymous_1);
                        let () = acc.push(_anonymous_0);
                        acc
                    },
                    variant_index: Some(const { ::core::num::NonZero::new(1).unwrap() }),
                }
            }
            Self::Lambda { body } => {
                ::pbt::reflection::Parts {
                    fields: {
                        let mut acc = ::pbt::fields::Store::new();
                        let () = acc.push(body);
                        acc
                    },
                    variant_index: Some(const { ::core::num::NonZero::new(2).unwrap() }),
                }
            }
            Self::Variable { de_bruijn } => {
                ::pbt::reflection::Parts {
                    fields: {
                        let mut acc = ::pbt::fields::Store::new();
                        let () = acc.push(de_bruijn);
                        acc
                    },
                    variant_index: Some(const { ::core::num::NonZero::new(3).unwrap() }),
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
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    let () = registration.register::<Box<Self>>();
                    let () = acc.insert(::core::any::TypeId::of::<Box<Self>>());
                    let () = registration.register::<Box<Self>>();
                    let () = acc.insert(::core::any::TypeId::of::<Box<Self>>());
                    acc
                },
            });
        let () = acc
            .push(::pbt::reflection::Variant {
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    let () = registration.register::<Box<Self>>();
                    let () = acc.insert(::core::any::TypeId::of::<Box<Self>>());
                    acc
                },
            });
        let () = acc
            .push(::pbt::reflection::Variant {
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    let () = registration.register::<usize>();
                    let () = acc.insert(::core::any::TypeId::of::<usize>());
                    acc
                },
            });
        ::pbt::reflection::Variants::Algebraic(acc)
    }
}
"#,
        );
    }

    #[test]
    fn generic() {
        expect_test(
            r#"
struct Generic<A, B, C>;
"#,
            derive_pbt,
            r#"
impl<A: ::pbt::Pbt, B: ::pbt::Pbt, C: ::pbt::Pbt> ::pbt::Pbt for Generic<A, B, C> {
    #[inline]
    fn construct<F>(
        ::pbt::reflection::Parts {
            mut fields,
            variant_index,
        }: ::pbt::reflection::Parts<F>,
    ) -> Self
    where
        F: ::pbt::fields::Fields,
    {
        let algebraic_index: usize = variant_index
            .expect("`Generic` is not a literal")
            .get();
        match algebraic_index {
            1 => Self,
            _ => panic!("can't instantiate variant #{algebraic_index} of `Generic`"),
        }
    }
    #[inline]
    fn deconstruct(self) -> ::pbt::reflection::Parts<::pbt::fields::Store> {
        match self {
            Self => {
                ::pbt::reflection::Parts {
                    fields: {
                        let mut acc = ::pbt::fields::Store::new();
                        acc
                    },
                    variant_index: Some(const { ::core::num::NonZero::new(1).unwrap() }),
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
                field_types: {
                    let mut acc = ::pbt::multiset::Multiset::new();
                    acc
                },
            });
        ::pbt::reflection::Variants::Algebraic(acc)
    }
}
"#,
        );
    }

    #[test]
    fn at_least_42() {
        expect_test(
            r#"
fn less_than_42(lc: &LambdaCalculus) {
    if let LambdaCalculus::Variable { de_bruijn } = *lc {
        assert!(de_bruijn < 42)
    }
}
"#,
            |ts| pbt(ts, 1_000_usize.into_token_stream()),
            r#"
#[test]
fn less_than_42() {
    let mut prng = ::pbt::WyRand::new(42);
    let maybe_witness = pbt::witness(
        |lc: &LambdaCalculus| -> Option<Option<String>> {
            ::pbt::panic::catch(move || {
                    if let LambdaCalculus::Variable { de_bruijn } = *lc {
                        assert!(de_bruijn < 42)
                    }
                })
                .err()
        },
        1_000,
        &mut prng,
    );
    if let Some((witness, maybe_panic_msg)) = maybe_witness {
        if let Some(panic_msg) = maybe_panic_msg {
            panic!(
                "\r\nConsider the following input:\r\n\r\n```\r\n{witness:#?}\r\n```\r\n\r\n{panic_msg}",
            );
        } else {
            panic!(
                "\r\nConsider the following input:\r\n\r\n```\r\n{witness:#?}\r\n```\r\n\r\nThis panicked, but the payload was not recoverable.",
            );
        }
    }
}
"#,
        );
    }

    #[test]
    fn lhs_at_most_rhs() {
        expect_test(
            r#"
fn lhs_at_most_rhs(lhs: &usize, rhs: &usize) {
    assert!(*lhs <= *rhs);
}
"#,
            |ts| pbt(ts, 1_000_usize.into_token_stream()),
            r#"
#[test]
fn lhs_at_most_rhs() {
    let mut prng = ::pbt::WyRand::new(42);
    let maybe_witness = pbt::witness(
        |&(ref lhs, ref rhs): &(usize, usize)| -> Option<Option<String>> {
            ::pbt::panic::catch(move || {
                    assert!(* lhs <= * rhs);
                })
                .err()
        },
        1_000,
        &mut prng,
    );
    if let Some((witness, maybe_panic_msg)) = maybe_witness {
        if let Some(panic_msg) = maybe_panic_msg {
            panic!(
                "\r\nConsider the following input:\r\n\r\n```\r\n{witness:#?}\r\n```\r\n\r\n{panic_msg}",
            );
        } else {
            panic!(
                "\r\nConsider the following input:\r\n\r\n```\r\n{witness:#?}\r\n```\r\n\r\nThis panicked, but the payload was not recoverable.",
            );
        }
    }
}
"#,
        );
    }
}
