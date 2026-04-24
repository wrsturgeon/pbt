#![allow(
    clippy::missing_inline_in_public_items,
    reason = "macros are the only public items"
)]
#![allow(
    clippy::missing_docs_in_private_items,
    clippy::single_call_fn,
    clippy::too_many_lines,
    reason = "writing macros is already hell"
)]

use {
    core::{iter, num::NonZero},
    proc_macro2::{Span, TokenStream},
    quote::{ToTokens as _, quote},
    syn::{
        AngleBracketedGenericArguments, Arm, Block, ConstParam, Expr, ExprArray, ExprBlock,
        ExprCall, ExprClosure, ExprLit, ExprMatch, ExprMethodCall, ExprPath, ExprStruct, Field,
        FieldPat, FieldValue, Fields, GenericArgument, GenericParam, Generics, Ident, Item,
        LifetimeParam, Lit, LitInt, Local, LocalInit, Macro, MacroDelimiter, Member, Pat, PatIdent,
        PatStruct, PatTuple, PatTupleStruct, Path, PathArguments, PathSegment, ReturnType, Stmt,
        Token, TraitBound, TraitBoundModifier, Type, TypeParam, TypeParamBound, TypePath,
        parse_macro_input,
        punctuated::Punctuated,
        spanned::Spanned as _,
        token::{Brace, Bracket, Paren, PathSep},
    },
};

/// Derive all necessary traits in the `pbt` crate.
/// # Panics
/// If the annotated item is neither an `enum` nor a `struct`.
#[proc_macro_derive(Pbt)]
pub fn derive_pbt(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match parse_macro_input!(ts as Item) {
        Item::Enum(item) => derive_pbt_for_ctors(
            item.ident,
            &item.generics,
            &item
                .variants
                .iter()
                .map(|variant| {
                    (
                        Path {
                            leading_colon: None,
                            segments: [seg(id("Self")), seg(variant.ident.clone())]
                                .into_iter()
                                .collect(),
                        },
                        &variant.fields,
                    )
                })
                .collect::<Vec<_>>(),
        ),
        Item::Struct(item) => derive_pbt_for_ctors(
            item.ident,
            &item.generics,
            &[(path_of_str("Self"), &item.fields)],
        ),
        item => syn::Error::into_compile_error(syn::Error::new(
            item.span(),
            "`#[derive(Pbt)]` expects an `enum` or a `struct`",
        )),
    }
    .into()
}

#[inline]
fn derive_pbt_for_ctors(
    ident: Ident,
    generics: &Generics,
    ctors: &[(Path, &Fields)],
) -> TokenStream {
    let construct_trait_path = Path {
        leading_colon: Some(PathSep::default()),
        segments: [seg(id("pbt")), seg(id("construct")), seg(id("Construct"))]
            .into_iter()
            .collect(),
    };
    let parameters = generics_to_parameters(generics);
    let generics = add_construct_bound_to_each_generic(generics, &construct_trait_path);
    let register_all_immediate_dependencies = register_all_immediate_dependencies(ctors);
    let elim_ctor_idx = elim_ctor_idx(ctors);
    let introduction_rules = Macro {
        path: path_of_str("vec"),
        bang_token: <Token![!]>::default(),
        delimiter: MacroDelimiter::Bracket(Bracket::default()),
        tokens: introduction_rules(ctors).into_token_stream(),
    };
    let visit_deep = visit(ctors, &id("visit_deep"));
    let test_mod_id = id(&format!("pbt_{ident}"));

    let impl_path = Path {
        leading_colon: None,
        segments: iter::once(PathSegment {
            ident: ident.clone(),
            arguments: PathArguments::AngleBracketed(parameters),
        })
        .collect(),
    };
    if ctors.is_empty() {
        let test_path = Path {
            leading_colon: None,
            segments: [
                seg(id("super")),
                PathSegment {
                    ident,
                    arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                        colon2_token: None,
                        lt_token: <Token![<]>::default(),
                        args: generics
                            .params
                            .iter()
                            .map(|_| {
                                GenericArgument::Type(Type::Path(TypePath {
                                    qself: None,
                                    path: path_of_str("usize"),
                                }))
                            })
                            .collect(),
                        gt_token: <Token![>]>::default(),
                    }),
                },
            ]
            .into_iter()
            .collect(),
        };
        return quote! {
            impl #generics #construct_trait_path for #impl_path {
                #[inline]
                fn register_all_immediate_dependencies(
                    visited: &mut ::std::collections::BTreeSet<::pbt::reflection::Type>,
                    _sccs: &mut ::pbt::StronglyConnectedComponents,
                ) {
                    let _ = visited.insert(::pbt::reflection::type_of::<Self>());
                }

                #[inline]
                fn type_former() -> ::pbt::construct::TypeFormer<Self> {
                    ::pbt::construct::TypeFormer::Algebraic(::pbt::construct::Algebraic {
                        introduction_rules: vec![],
                        elimination_rule: ::pbt::construct::ElimFn::new(|uninhabited| match uninhabited {}),
                    })
                }

                #[inline]
                fn visit_deep<V: ::pbt::construct::Construct>(&self) -> impl ::core::iter::Iterator<Item = V> {
                    ::core::iter::empty()
                }
            }

            #[cfg(test)]
            mod #test_mod_id {
                #[test]
                fn eta_expansion() {
                    let () = ::pbt::construct::check_eta_expansion::<#test_path>();
                }

                #[test]
                fn serialization_roundtrip() {
                    let () = ::pbt::cache::check_roundtrip::<#test_path>();
                }
            }
        };
    }
    let test_path = Path {
        leading_colon: None,
        segments: [
            seg(id("super")),
            PathSegment {
                ident,
                arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    colon2_token: None,
                    lt_token: <Token![<]>::default(),
                    args: generics
                        .params
                        .iter()
                        .map(|_| {
                            GenericArgument::Type(Type::Path(TypePath {
                                qself: None,
                                path: path_of_str("usize"),
                            }))
                        })
                        .collect(),
                    gt_token: <Token![>]>::default(),
                }),
            },
        ]
        .into_iter()
        .collect(),
    };

    quote! {
        impl #generics #construct_trait_path for #impl_path {
            #[inline]
            fn register_all_immediate_dependencies(
                visited: &mut ::std::collections::BTreeSet<::pbt::reflection::Type>,
                sccs: &mut ::pbt::StronglyConnectedComponents,
            ) {
                if !visited.insert(::pbt::reflection::type_of::<Self>()) {
                    return;
                }
                #register_all_immediate_dependencies
            }

            #[inline]
            fn type_former() -> ::pbt::construct::TypeFormer<Self> {
                ::pbt::construct::TypeFormer::Algebraic(::pbt::construct::Algebraic {
                    introduction_rules: #introduction_rules,
                    elimination_rule: ::pbt::construct::ElimFn::new(|constructed| {
                        let mut fields = ::pbt::reflection::TermsOfVariousTypes::new();
                        let ctor_idx: usize = #elim_ctor_idx;
                        ::pbt::construct::Decomposition {
                            // SAFETY: Case anaylsis above.
                            ctor_idx: unsafe { ::core::num::NonZero::new_unchecked(ctor_idx) },
                            fields,
                        }
                    }),
                })
            }

            #[inline]
            fn visit_deep<V: ::pbt::construct::Construct>(&self) -> impl ::core::iter::Iterator<Item = V> {
                ::pbt::construct::visit_self(self).chain({
                    let iter: Box<dyn Iterator<Item = _>> = #visit_deep;
                    iter
                })
            }
        }

        #[cfg(test)]
        mod #test_mod_id {
            #[test]
            fn eta_expansion() {
                let () = ::pbt::construct::check_eta_expansion::<#test_path>();
            }

            #[test]
            fn serialization_roundtrip() {
                let () = ::pbt::cache::check_roundtrip::<#test_path>();
            }
        }
    }
}

#[inline]
fn add_construct_bound_to_each_generic(
    generics: &Generics,
    construct_trait_path: &Path,
) -> Generics {
    let Generics {
        lt_token,
        ref params,
        gt_token,
        ref where_clause,
    } = *generics;
    Generics {
        lt_token,
        params: params
            .iter()
            .map(|p| {
                let GenericParam::Type(TypeParam {
                    ref ident,
                    colon_token,
                    ref bounds,
                    eq_token,
                    ..
                }) = *p
                else {
                    return p.clone();
                };
                GenericParam::Type(TypeParam {
                    attrs: vec![],
                    ident: ident.clone(),
                    colon_token,
                    bounds: bounds
                        .iter()
                        .cloned()
                        .chain(iter::once(TypeParamBound::Trait(TraitBound {
                            paren_token: None,
                            modifier: TraitBoundModifier::None,
                            lifetimes: None,
                            path: construct_trait_path.clone(),
                        })))
                        .collect(),
                    eq_token,
                    default: None,
                })
            })
            .collect(),
        gt_token,
        where_clause: where_clause.clone(),
    }
}

#[inline]
fn generics_to_parameters(generics: &Generics) -> AngleBracketedGenericArguments {
    let Generics {
        lt_token,
        ref params,
        gt_token,
        ..
    } = *generics;
    AngleBracketedGenericArguments {
        colon2_token: None,
        lt_token: lt_token.unwrap_or_else(Default::default),
        args: params
            .iter()
            .map(|p| -> GenericArgument {
                match *p {
                    GenericParam::Const(ConstParam { ref ident, .. }) => {
                        GenericArgument::Const(Expr::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: path_of_id(ident.clone()),
                        }))
                    }
                    GenericParam::Lifetime(LifetimeParam { ref lifetime, .. }) => {
                        GenericArgument::Lifetime(lifetime.clone())
                    }
                    GenericParam::Type(TypeParam { ref ident, .. }) => {
                        GenericArgument::Type(Type::Path(TypePath {
                            qself: None,
                            path: path_of_id(ident.clone()),
                        }))
                    }
                }
            })
            .collect(),
        gt_token: gt_token.unwrap_or_else(Default::default),
    }
}

#[inline]
fn register_all_immediate_dependencies(ctors: &[(Path, &Fields)]) -> Block {
    Block {
        brace_token: Brace::default(),
        stmts: ctors
            .iter()
            .flat_map(|&(_, fields)| {
                fields.iter().map(|&Field { ref ty, .. }| {
                    Stmt::Local(Local {
                        attrs: vec![],
                        let_token: <Token![let]>::default(),
                        pat: Pat::Tuple(PatTuple {
                            attrs: vec![],
                            paren_token: Paren::default(),
                            elems: Punctuated::new(),
                        }),
                        init: Some(LocalInit {
                            eq_token: <Token![=]>::default(),
                            expr: Box::new(Expr::Verbatim(quote! {
                                ::pbt::reflection::register::<#ty>(visited.clone(), sccs)
                            })),
                            diverge: None,
                        }),
                        semi_token: <Token![;]>::default(),
                    })
                })
            })
            .collect(),
    }
}

#[inline]
fn introduction_rules(ctors: &[(Path, &Fields)]) -> Punctuated<Expr, Token![,]> {
    ctors
        .iter()
        .map(|&(ref path, fields)| -> Expr {
            Expr::Struct(ExprStruct {
                attrs: vec![],
                qself: None,
                path: Path {
                    leading_colon: Some(PathSep::default()),
                    segments: [
                        seg(id("pbt")),
                        seg(id("construct")),
                        seg(id("IntroductionRule")),
                    ]
                    .into_iter()
                    .collect(),
                },
                brace_token: Brace::default(),
                fields: [
                    FieldValue {
                        attrs: vec![],
                        colon_token: Some(<Token![:]>::default()),
                        member: Member::Named(id("arbitrary_fields")),
                        expr: Expr::Closure(ExprClosure {
                            attrs: vec![],
                            lifetimes: None,
                            constness: None,
                            movability: None,
                            asyncness: None,
                            capture: None,
                            or1_token: <Token![|]>::default(),
                            inputs: [
                                Pat::Ident(PatIdent {
                                    attrs: vec![],
                                    by_ref: None,
                                    mutability: None,
                                    ident: id("prng"),
                                    subpat: None,
                                }),
                                Pat::Ident(PatIdent {
                                    attrs: vec![],
                                    by_ref: None,
                                    mutability: Some(<Token![mut]>::default()),
                                    ident: id("sizes"),
                                    subpat: None,
                                }),
                            ]
                            .into_iter()
                            .collect(),
                            or2_token: <Token![|]>::default(),
                            output: ReturnType::Default,
                            body: Box::new(Expr::Block(ExprBlock {
                                attrs: vec![],
                                label: None,
                                block: Block {
                                    brace_token: Brace::default(),
                                    stmts: iter::once(Stmt::Local(Local {
                                        attrs: vec![],
                                        let_token: <Token![let]>::default(),
                                        pat: Pat::Ident(PatIdent {
                                            attrs: vec![],
                                            by_ref: None,
                                            mutability: Some(<Token![mut]>::default()),
                                            ident: id("fields"),
                                            subpat: None,
                                        }),
                                        init: Some(LocalInit {
                                            eq_token: <Token![=]>::default(),
                                            expr: Box::new(Expr::Call(ExprCall {
                                                attrs: vec![],
                                                func: Box::new(Expr::Path(ExprPath {
                                                    attrs: vec![],
                                                    qself: None,
                                                    path: Path {
                                                        leading_colon: Some(PathSep::default()),
                                                        segments: [
                                                            seg(id("pbt")),
                                                            seg(id("reflection")),
                                                            seg(id("TermsOfVariousTypes")),
                                                            seg(id("new")),
                                                        ]
                                                        .into_iter()
                                                        .collect(),
                                                    },
                                                })),
                                                paren_token: Paren::default(),
                                                args: Punctuated::new(),
                                            })),
                                            diverge: None,
                                        }),
                                        semi_token: <Token![;]>::default(),
                                    }))
                                    .chain(fields.iter().map(|&Field { ref ty, .. }| {
                                        Stmt::Local(Local {
                                            attrs: vec![],
                                            let_token: <Token![let]>::default(),
                                            pat: Pat::Tuple(PatTuple {
                                                attrs: vec![],
                                                paren_token: Paren::default(),
                                                elems: Punctuated::new(),
                                            }),
                                            init: Some(LocalInit {
                                                eq_token: <Token![=]>::default(),
                                                expr: Box::new(Expr::Verbatim(quote! {
                                                   fields.push(sizes.arbitrary::<#ty>(prng))
                                                })),
                                                diverge: None,
                                            }),
                                            semi_token: <Token![;]>::default(),
                                        })
                                    }))
                                    .chain(iter::once(Stmt::Expr(
                                        Expr::Path(ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path: path_of_str("fields"),
                                        }),
                                        None,
                                    )))
                                    .collect(),
                                },
                            })),
                        }),
                    },
                    FieldValue {
                        attrs: vec![],
                        colon_token: Some(<Token![:]>::default()),
                        member: Member::Named(id("call")),
                        expr: Expr::Call(ExprCall {
                            attrs: vec![],
                            func: Box::new(Expr::Verbatim(
                                quote! { ::pbt::construct::CtorFn::new },
                            )),
                            paren_token: Paren::default(),
                            args: iter::once(Expr::Closure(ExprClosure {
                                attrs: vec![],
                                lifetimes: None,
                                constness: None,
                                movability: None,
                                asyncness: None,
                                capture: None,
                                or1_token: <Token![|]>::default(),
                                inputs: iter::once(Pat::Ident(PatIdent {
                                    attrs: vec![],
                                    by_ref: None,
                                    mutability: None,
                                    ident: id("terms"),
                                    subpat: None,
                                }))
                                .collect(),
                                or2_token: <Token![|]>::default(),
                                output: ReturnType::Default,
                                body: {
                                    let some = match *fields {
                                        Fields::Unit => Expr::Path(ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path: path.clone(),
                                        }),
                                        Fields::Unnamed(ref fields) => Expr::Call(ExprCall {
                                            attrs: vec![],
                                            func: Box::new(Expr::Path(ExprPath {
                                                attrs: vec![],
                                                qself: None,
                                                path: path.clone(),
                                            })),
                                            paren_token: fields.paren_token,
                                            args: fields
                                                .unnamed
                                                .iter()
                                                .map(|&Field { ref ty, .. }| {
                                                    Expr::Verbatim(quote! {
                                                        terms.must_pop::<#ty>()
                                                    })
                                                })
                                                .collect(),
                                        }),
                                        Fields::Named(ref fields) => Expr::Struct(ExprStruct {
                                            attrs: vec![],
                                            qself: None,
                                            path: path.clone(),
                                            brace_token: fields.brace_token,
                                            fields: fields
                                                .named
                                                .iter()
                                                .enumerate()
                                                .map(
                                                    |(
                                                        i,
                                                        &Field {
                                                            ref ident, ref ty, ..
                                                        },
                                                    )| {
                                                        let ident = force_ident(ident.as_ref(), i);
                                                        FieldValue {
                                                            attrs: vec![],
                                                            member: Member::Named(ident),
                                                            colon_token: Some(<Token![:]>::default()),
                                                            expr: Expr::Verbatim(quote! {
                                                                terms.must_pop::<#ty>()
                                                            }),
                                                        }
                                                    },
                                                )
                                                .collect(),
                                            dot2_token: None,
                                            rest: None,
                                        }),
                                    };
                                    Box::new(Expr::Call(ExprCall {
                                        attrs: vec![],
                                        func: Box::new(Expr::Path(ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path: path_of_str("Some"),
                                        })),
                                        paren_token: Paren::default(),
                                        args: iter::once(some).collect(),
                                    }))
                                },
                            }))
                            .collect(),
                        }),
                    },
                    FieldValue {
                        attrs: vec![],
                        colon_token: Some(<Token![:]>::default()),
                        member: Member::Named(id("immediate_dependencies")),
                        expr: {
                            let array = Expr::Array(ExprArray {
                                attrs: vec![],
                                bracket_token: Bracket::default(),
                                elems: fields
                                    .iter()
                                    .map(|&Field { ref ty, .. }| {
                                        Expr::Verbatim(quote! {
                                            ::pbt::reflection::type_of::<#ty>()
                                        })
                                    })
                                    .collect(),
                            });
                            Expr::Verbatim(quote! { #array.into_iter().collect() })
                        },
                    },
                ]
                .into_iter()
                .collect(),
                dot2_token: None,
                rest: None,
            })
        })
        .collect()
}

#[inline]
fn elim_ctor_idx(ctors: &[(Path, &Fields)]) -> ExprMatch {
    ExprMatch {
        attrs: vec![],
        match_token: <Token![match]>::default(),
        expr: Box::new(Expr::Verbatim(quote! { constructed })),
        brace_token: Brace::default(),
        arms: ctors
            .iter()
            .enumerate()
            .map(|(index, &(ref path, fields))| {
                // SAFETY: Adding 1.
                let index = unsafe {
                    NonZero::new_unchecked(
                        #[expect(clippy::expect_used, reason = "extremely unlikely")]
                        index
                            .checked_add(1)
                            .expect("internal `pbt` error: more than `usize::MAX` constructors"),
                    )
                };
                Arm {
                    attrs: vec![],
                    pat: match *fields {
                        Fields::Unit => Pat::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                        }),
                        Fields::Named(ref fields) => Pat::Struct(PatStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            brace_token: fields.brace_token,
                            fields: fields
                                .named
                                .iter()
                                .enumerate()
                                .map(|(i, field)| FieldPat {
                                    attrs: vec![],
                                    member: Member::Named(force_ident(field.ident.as_ref(), i)),
                                    colon_token: None,
                                    pat: Box::new(Pat::Ident(PatIdent {
                                        attrs: vec![],
                                        by_ref: None,
                                        mutability: None,
                                        ident: force_ident(field.ident.as_ref(), i),
                                        subpat: None,
                                    })),
                                })
                                .collect(),
                            rest: None,
                        }),
                        Fields::Unnamed(ref fields) => Pat::TupleStruct(PatTupleStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            paren_token: fields.paren_token,
                            elems: fields
                                .unnamed
                                .iter()
                                .enumerate()
                                .map(|(i, field)| {
                                    Pat::Ident(PatIdent {
                                        attrs: vec![],
                                        by_ref: None,
                                        mutability: None,
                                        ident: force_ident(field.ident.as_ref(), i),
                                        subpat: None,
                                    })
                                })
                                .collect(),
                        }),
                    },
                    guard: None,
                    fat_arrow_token: <Token![=>]>::default(),
                    body: Box::new(Expr::Block(ExprBlock {
                        attrs: vec![],
                        label: None,
                        block: Block {
                            brace_token: Brace::default(),
                            stmts: fields
                                .iter()
                                .enumerate()
                                .rev()
                                .map(
                                    |(
                                        i,
                                        &Field {
                                            ref ident, ref ty, ..
                                        },
                                    )| {
                                        let ident = force_ident(ident.as_ref(), i);
                                        Stmt::Local(Local {
                                            attrs: vec![],
                                            let_token: <Token![let]>::default(),
                                            pat: Pat::Tuple(PatTuple {
                                                attrs: vec![],
                                                paren_token: Paren::default(),
                                                elems: Punctuated::new(),
                                            }),
                                            init: Some(LocalInit {
                                                eq_token: <Token![=]>::default(),
                                                expr: Box::new(Expr::Verbatim(quote! {
                                                    fields.push::<#ty>(#ident)
                                                })),
                                                diverge: None,
                                            }),
                                            semi_token: <Token![;]>::default(),
                                        })
                                    },
                                )
                                .chain(iter::once(Stmt::Expr(
                                    Expr::Lit(ExprLit {
                                        attrs: vec![],
                                        lit: Lit::Int(LitInt::new(
                                            &index.to_string(),
                                            Span::call_site(),
                                        )),
                                    }),
                                    None,
                                )))
                                .collect(),
                        },
                    })),
                    comma: Some(<Token![,]>::default()),
                }
            })
            .collect(),
    }
}

#[inline]
fn visit(ctors: &[(Path, &Fields)], visit_fn: &Ident) -> ExprMatch {
    ExprMatch {
        attrs: vec![],
        match_token: <Token![match]>::default(),
        expr: Box::new(Expr::Verbatim(quote! { self })),
        brace_token: Brace::default(),
        arms: ctors
            .iter()
            .map(|&(ref path, fields)| Arm {
                attrs: vec![],
                pat: match *fields {
                    Fields::Unit => Pat::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: path.clone(),
                    }),
                    Fields::Named(ref fields) => Pat::Struct(PatStruct {
                        attrs: vec![],
                        qself: None,
                        path: path.clone(),
                        brace_token: fields.brace_token,
                        fields: fields
                            .named
                            .iter()
                            .enumerate()
                            .map(|(i, field)| FieldPat {
                                attrs: vec![],
                                member: Member::Named(force_ident(field.ident.as_ref(), i)),
                                colon_token: None,
                                pat: Box::new(Pat::Ident(PatIdent {
                                    attrs: vec![],
                                    by_ref: None,
                                    mutability: None,
                                    ident: force_ident(field.ident.as_ref(), i),
                                    subpat: None,
                                })),
                            })
                            .collect(),
                        rest: None,
                    }),
                    Fields::Unnamed(ref fields) => Pat::TupleStruct(PatTupleStruct {
                        attrs: vec![],
                        qself: None,
                        path: path.clone(),
                        paren_token: fields.paren_token,
                        elems: fields
                            .unnamed
                            .iter()
                            .enumerate()
                            .map(|(i, field)| {
                                Pat::Ident(PatIdent {
                                    attrs: vec![],
                                    by_ref: None,
                                    mutability: None,
                                    ident: force_ident(field.ident.as_ref(), i),
                                    subpat: None,
                                })
                            })
                            .collect(),
                    }),
                },
                guard: None,
                fat_arrow_token: <Token![=>]>::default(),
                body: {
                    let iter = fields.iter().enumerate().fold(
                        Expr::Verbatim(quote! { ::core::iter::empty() }),
                        |acc, (i, &Field { ref ident, .. })| {
                            let ident = force_ident(ident.as_ref(), i);
                            Expr::MethodCall(ExprMethodCall {
                                attrs: vec![],
                                receiver: Box::new(Expr::MethodCall(ExprMethodCall {
                                    attrs: vec![],
                                    receiver: Box::new(Expr::Path(ExprPath {
                                        attrs: vec![],
                                        qself: None,
                                        path: Path {
                                            leading_colon: None,
                                            segments: iter::once(seg(ident)).collect(),
                                        },
                                    })),
                                    dot_token: <Token![.]>::default(),
                                    method: visit_fn.clone(),
                                    turbofish: None,
                                    paren_token: Paren::default(),
                                    args: Punctuated::new(),
                                })),
                                dot_token: <Token![.]>::default(),
                                method: id("chain"),
                                turbofish: None,
                                paren_token: Paren::default(),
                                args: iter::once(acc).collect(),
                            })
                        },
                    );
                    Box::new(Expr::Verbatim(quote! {
                        Box::new(#iter)
                    }))
                },
                comma: Some(<Token![,]>::default()),
            })
            .collect(),
    }
}

#[inline]
fn path_of_id(ident: Ident) -> Path {
    Path {
        leading_colon: None,
        segments: iter::once(seg(ident)).collect(),
    }
}

#[inline]
fn path_of_str(str: &str) -> Path {
    path_of_id(id(str))
}

#[inline]
fn id(str: &str) -> Ident {
    Ident::new(str, Span::call_site())
}

#[inline]
fn seg(ident: Ident) -> PathSegment {
    PathSegment {
        ident,
        arguments: PathArguments::None,
    }
}

#[inline]
fn force_ident(maybe_id: Option<&Ident>, index: usize) -> Ident {
    maybe_id.map_or_else(|| id(&format!("_{index}")), Clone::clone)
}
