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
    core::iter,
    proc_macro::TokenStream,
    proc_macro2::Span,
    quote::quote,
    syn::{
        AngleBracketedGenericArguments, Arm, BareFnArg, BinOp, Block, ConstParam, Expr, ExprArray,
        ExprBinary, ExprBlock, ExprCall, ExprCast, ExprClosure, ExprForLoop, ExprIf, ExprMatch,
        ExprMethodCall, ExprPath, ExprReference, ExprReturn, ExprStruct, ExprTry, ExprTuple,
        ExprUnary, ExprUnsafe, FieldPat, FieldValue, Fields, GenericArgument, GenericParam,
        Generics, Ident, Item, ItemEnum, ItemStruct, LifetimeParam, Local, LocalInit, Member, Pat,
        PatIdent, PatPath, PatStruct, PatTuple, PatTupleStruct, PatType, Path, PathArguments,
        PathSegment, QSelf, ReturnType, Stmt, Token, TraitBound, TraitBoundModifier, Type,
        TypeBareFn, TypeInfer, TypeParam, TypeParamBound, TypePath, TypeTuple, UnOp, Variant,
        fold::{Fold, fold_type_path},
        parse_macro_input,
        punctuated::Punctuated,
        token::{Brace, Bracket, Paren},
    },
};

struct ReplaceSelfWithInductive;

#[expect(clippy::missing_trait_methods, reason = "infeasible")]
impl Fold for ReplaceSelfWithInductive {
    #[inline]
    fn fold_type_path(&mut self, i: TypePath) -> TypePath {
        let mut segments = i.path.segments.iter();
        if let Some(head) = segments.next()
            && head.ident == "Self"
            && segments.next().is_none()
        {
            TypePath {
                qself: None,
                path: Path {
                    leading_colon: Some(<Token![::]>::default()),
                    segments: [seg(id("pbt")), seg(id("count")), seg(id("Inductive"))]
                        .into_iter()
                        .collect(),
                },
            }
        } else {
            fold_type_path(self, i)
        }
    }
}

#[proc_macro_derive(Pbt)]
pub fn derive_pbt(ts: TokenStream) -> TokenStream {
    match parse_macro_input!(ts as Item) {
        Item::Enum(ref item) => derive_pbt_for_enum(item),
        Item::Struct(ref item) => derive_pbt_for_struct(item),
        _ => panic!("expected an `enum` or a `struct`"),
    }
}

#[inline]
fn derive_pbt_for_enum(item: &ItemEnum) -> TokenStream {
    let ItemEnum {
        ref ident,
        ref generics,
        ..
    } = *item;

    let count_trait = Path {
        leading_colon: Some(<Token![::]>::default()),
        segments: [seg(id("pbt")), seg(id("count")), seg(id("Count"))]
            .into_iter()
            .collect(),
    };
    let conjure_trait = Path {
        leading_colon: Some(<Token![::]>::default()),
        segments: [seg(id("pbt")), seg(id("conjure")), seg(id("Conjure"))]
            .into_iter()
            .collect(),
    };
    let shrink_trait = Path {
        leading_colon: Some(<Token![::]>::default()),
        segments: [seg(id("pbt")), seg(id("shrink")), seg(id("Shrink"))]
            .into_iter()
            .collect(),
    };
    let [count_params, conjure_params, shrink_params] = {
        [
            count_trait.clone(),
            conjure_trait.clone(),
            shrink_trait.clone(),
        ]
        .map(|path| {
            add_extra_bound(
                generics.clone(),
                &TypeParamBound::Trait(TraitBound {
                    paren_token: None,
                    modifier: TraitBoundModifier::None,
                    lifetimes: None,
                    path,
                }),
            )
        })
    };
    let ty_args = PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        colon2_token: None,
        lt_token: <Token![<]>::default(),
        args: params_to_args(&generics.params),
        gt_token: <Token![>]>::default(),
    });

    let cardinality = cardinality_body_for_enum(item);
    let conjure = conjure_body_for_enum();
    let corners = corners_body_for_enum(item);
    let variants = variants_body_for_enum(item);
    let leaf = leaf_body_for_enum(item);
    let step = step_body_for_enum(item);

    quote! {
        impl #count_params #count_trait for #ident #ty_args {
            const CARDINALITY: ::pbt::count::Cardinality = #cardinality;
        }

        impl #conjure_params #conjure_trait for #ident #ty_args {
            #[inline]
            fn conjure(mut seed: ::pbt::conjure::Seed) -> Result<Self, ::pbt::conjure::Uninstantiable> #conjure

            #[inline]
            fn corners() -> Box<dyn Iterator<Item = Self>> #corners

            #[inline]
            fn variants() -> impl Iterator<Item = (::pbt::count::Cardinality, fn(::pbt::conjure::Seed) -> Self)> #variants

            #[inline]
            fn leaf(mut seed: ::pbt::conjure::Seed) -> Result<Self, ::pbt::conjure::Uninstantiable> #leaf
        }

        impl #shrink_params #shrink_trait for #ident #ty_args {
            #[inline]
            fn step<P: for<'s> FnMut(&'s Self) -> bool + ?Sized>(&self, property: &mut P) -> Option<Self> #step
        }
    }
    .into()
}

#[inline]
fn derive_pbt_for_struct(item: &ItemStruct) -> TokenStream {
    let ItemStruct {
        ref ident,
        ref generics,
        ..
    } = *item;

    let count_trait = Path {
        leading_colon: Some(<Token![::]>::default()),
        segments: [seg(id("pbt")), seg(id("count")), seg(id("Count"))]
            .into_iter()
            .collect(),
    };
    let conjure_trait = Path {
        leading_colon: Some(<Token![::]>::default()),
        segments: [seg(id("pbt")), seg(id("conjure")), seg(id("Conjure"))]
            .into_iter()
            .collect(),
    };
    let shrink_trait = Path {
        leading_colon: Some(<Token![::]>::default()),
        segments: [seg(id("pbt")), seg(id("shrink")), seg(id("Shrink"))]
            .into_iter()
            .collect(),
    };
    let [count_params, conjure_params, shrink_params] = {
        [
            count_trait.clone(),
            conjure_trait.clone(),
            shrink_trait.clone(),
        ]
        .map(|path| {
            add_extra_bound(
                generics.clone(),
                &TypeParamBound::Trait(TraitBound {
                    paren_token: None,
                    modifier: TraitBoundModifier::None,
                    lifetimes: None,
                    path,
                }),
            )
        })
    };
    let ty_args = PathArguments::AngleBracketed(AngleBracketedGenericArguments {
        colon2_token: None,
        lt_token: <Token![<]>::default(),
        args: params_to_args(&generics.params),
        gt_token: <Token![>]>::default(),
    });

    let cardinality = cardinality_body_for_struct(item);
    let conjure = conjure_body_for_struct(item);
    let corners = corners_body_for_struct(item);
    let variants = variants_body_for_struct();
    let leaf = leaf_body_for_struct(item);
    let step = step_body_for_struct(item);

    quote! {
        impl #count_params #count_trait for #ident #ty_args {
            const CARDINALITY: ::pbt::count::Cardinality = #cardinality;
        }

        impl #conjure_params #conjure_trait for #ident #ty_args {
            #[inline]
            fn conjure(mut seed: ::pbt::conjure::Seed) -> Result<Self, ::pbt::conjure::Uninstantiable> #conjure

            #[inline]
            fn corners() -> Box<dyn Iterator<Item = Self>> #corners

            #[inline]
            fn variants() -> impl Iterator<Item = (::pbt::count::Cardinality, fn(::pbt::conjure::Seed) -> Self)> #variants

            #[inline]
            fn leaf(mut seed: ::pbt::conjure::Seed) -> Result<Self, ::pbt::conjure::Uninstantiable> #leaf
        }

        impl #shrink_params #shrink_trait for #ident #ty_args {
            #[inline]
            fn step<P: for<'s> FnMut(&'s Self) -> bool + ?Sized>(&self, property: &mut P) -> Option<Self> #step
        }
    }
    .into()
}

#[inline]
fn force_id(maybe_id: Option<&Ident>, index: usize) -> Ident {
    maybe_id.map_or_else(|| id(&format!("_{index}")), Clone::clone)
}

#[inline]
fn cardinality_body_for_enum(item: &ItemEnum) -> Expr {
    item.variants.iter().fold(
        Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: Some(<Token![::]>::default()),
                segments: [
                    seg(id("pbt")),
                    seg(id("count")),
                    seg(id("Cardinality")),
                    seg(id("Empty")),
                ]
                .into_iter()
                .collect(),
            },
        }),
        |acc, variant| {
            Expr::MethodCall(ExprMethodCall {
                attrs: vec![],
                receiver: Box::new(acc),
                dot_token: <Token![.]>::default(),
                method: id("of_sum"),
                turbofish: None,
                paren_token: Paren::default(),
                args: iter::once(cardinality_of_variant(variant)).collect(),
            })
        },
    )
}

#[inline]
fn cardinality_body_for_struct(item: &ItemStruct) -> Expr {
    item.fields.iter().fold(
        Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: Some(<Token![::]>::default()),
                segments: [
                    seg(id("pbt")),
                    seg(id("count")),
                    seg(id("Cardinality")),
                    seg(id("Finite")),
                ]
                .into_iter()
                .collect(),
            },
        }),
        |acc, field| {
            Expr::MethodCall(ExprMethodCall {
                attrs: vec![],
                receiver: Box::new(acc),
                dot_token: <Token![.]>::default(),
                method: id("of_prod"),
                turbofish: None,
                paren_token: Paren::default(),
                args: iter::once(Expr::Path(ExprPath {
                    attrs: vec![],
                    qself: Some(QSelf {
                        lt_token: <Token![<]>::default(),
                        ty: Box::new(ReplaceSelfWithInductive.fold_type(field.ty.clone())),
                        position: 3,
                        as_token: Some(<Token![as]>::default()),
                        gt_token: <Token![>]>::default(),
                    }),
                    path: Path {
                        leading_colon: Some(<Token![::]>::default()),
                        segments: [
                            seg(id("pbt")),
                            seg(id("count")),
                            seg(id("Count")),
                            seg(id("CARDINALITY")),
                        ]
                        .into_iter()
                        .collect(),
                    },
                }))
                .collect(),
            })
        },
    )
}

#[inline]
fn tuples_of_fields(fields: &Fields) -> (Type, Pat) {
    (
        Type::Tuple(TypeTuple {
            paren_token: Paren::default(),
            elems: fields.iter().map(|field| field.ty.clone()).collect(),
        }),
        Pat::Tuple(PatTuple {
            attrs: vec![],
            paren_token: Paren::default(),
            elems: fields
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    Pat::Ident(PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: force_id(field.ident.as_ref(), i),
                        subpat: None,
                    })
                })
                .collect(),
        }),
    )
}

#[inline]
fn tuples_of_variant(variant: &Variant) -> (Type, Pat) {
    tuples_of_fields(&variant.fields)
}

#[inline]
fn instantiate_fields(path: Path, fields: &Fields) -> Expr {
    match *fields {
        Fields::Named(ref fields) => Expr::Struct(ExprStruct {
            attrs: vec![],
            qself: None,
            path,
            brace_token: Brace::default(),
            fields: fields
                .named
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    let id = force_id(field.ident.as_ref(), i);
                    FieldValue {
                        attrs: vec![],
                        member: Member::Named(id.clone()),
                        colon_token: Some(<Token![:]>::default()),
                        expr: Expr::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: path_of_id(id),
                        }),
                    }
                })
                .collect(),
            dot2_token: None,
            rest: None,
        }),
        Fields::Unit => Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path,
        }),
        Fields::Unnamed(ref fields) => Expr::Call(ExprCall {
            attrs: vec![],
            func: Box::new(Expr::Path(ExprPath {
                attrs: vec![],
                qself: None,
                path,
            })),
            paren_token: Paren::default(),
            args: fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: path_of_id(force_id(field.ident.as_ref(), i)),
                    })
                })
                .collect(),
        }),
    }
}

#[inline]
fn instantiate_variant(variant: &Variant) -> Expr {
    instantiate_fields(
        Path {
            leading_colon: None,
            segments: [seg(id("Self")), seg(variant.ident.clone())]
                .into_iter()
                .collect(),
        },
        &variant.fields,
    )
}

#[inline]
fn variants_body_for_enum(item: &ItemEnum) -> Block {
    let seed_type = Type::Path(TypePath {
        qself: None,
        path: Path {
            leading_colon: Some(<Token![::]>::default()),
            segments: [seg(id("pbt")), seg(id("conjure")), seg(id("Seed"))]
                .into_iter()
                .collect(),
        },
    });
    let fn_type = Type::BareFn(TypeBareFn {
        lifetimes: None,
        unsafety: None,
        abi: None,
        fn_token: <Token![fn]>::default(),
        paren_token: Paren::default(),
        inputs: iter::once(BareFnArg {
            attrs: vec![],
            name: None,
            ty: seed_type.clone(),
        })
        .collect(),
        variadic: None,
        output: ReturnType::Type(
            <Token![->]>::default(),
            Box::new(Type::Path(TypePath {
                qself: None,
                path: path_of_str("Self"),
            })),
        ),
    });
    let element_per_variant = |variant: &Variant| -> Expr {
        let (tuple_type, tuple_pat) = tuples_of_variant(variant);
        Expr::Tuple(ExprTuple {
            attrs: vec![],
            paren_token: Paren::default(),
            elems: [
                Expr::Path(ExprPath {
                    attrs: vec![],
                    qself: Some(QSelf {
                        lt_token: <Token![<]>::default(),
                        ty: Box::new(tuple_type.clone()),
                        position: 3,
                        as_token: Some(<Token![as]>::default()),
                        gt_token: <Token![>]>::default(),
                    }),
                    path: Path {
                        leading_colon: Some(<Token![::]>::default()),
                        segments: [
                            seg(id("pbt")),
                            seg(id("count")),
                            seg(id("Count")),
                            seg(id("CARDINALITY")),
                        ]
                        .into_iter()
                        .collect(),
                    },
                }),
                /*
                Expr::Cast(ExprCast {
                    attrs: vec![],
                    expr: Box::new(Expr::Closure(ExprClosure {
                        attrs: vec![],
                        lifetimes: None,
                        constness: None,
                        movability: None,
                        asyncness: None,
                        capture: None,
                        or1_token: <Token![|]>::default(),
                        inputs: iter::once(Pat::Type(PatType {
                            attrs: vec![],
                            pat: Box::new(Pat::Path(PatPath {
                                attrs: vec![],
                                qself: None,
                                path: path_of_str("seed"),
                            })),
                            colon_token: <Token![:]>::default(),
                            ty: Box::new(seed_type.clone()),
                        }))
                        .collect(),
                        or2_token: <Token![|]>::default(),
                        output: ReturnType::Type(
                            <Token![->]>::default(),
                            Box::new(Type::Path(TypePath {
                                qself: None,
                                path: Path {
                                    leading_colon: None,
                                    segments: iter::once(PathSegment {
                                        ident: id("Result"),
                                        arguments: PathArguments::AngleBracketed(
                                            AngleBracketedGenericArguments {
                                                colon2_token: None,
                                                lt_token: <Token![<]>::default(),
                                                args: [
                                                    GenericArgument::Type(Type::Path(TypePath {
                                                        qself: None,
                                                        path: path_of_str("Self"),
                                                    })),
                                                    GenericArgument::Type(Type::Path(TypePath {
                                                        qself: None,
                                                        path: Path {
                                                            leading_colon: Some(
                                                                <Token![::]>::default(),
                                                            ),
                                                            segments: [
                                                                seg(id("pbt")),
                                                                seg(id("conjure")),
                                                                seg(id("Uninstantiable")),
                                                            ]
                                                            .into_iter()
                                                            .collect(),
                                                        },
                                                    })),
                                                ]
                                                .into_iter()
                                                .collect(),
                                                gt_token: <Token![>]>::default(),
                                            },
                                        ),
                                    })
                                    .collect(),
                                },
                            })),
                        ),
                        body: Box::new(Expr::Block(ExprBlock {
                            attrs: vec![],
                            label: None,
                            block: Block {
                                brace_token: Brace::default(),
                                stmts: vec![
                                    Stmt::Local(Local {
                                        attrs: vec![],
                                        let_token: <Token![let]>::default(),
                                        pat: tuple_pat,
                                        init: Some(LocalInit {
                                            eq_token: <Token![=]>::default(),
                                            expr: Box::new(Expr::Try(ExprTry {
                                                attrs: vec![],
                                                expr: Box::new(Expr::Call(ExprCall {
                                                    attrs: vec![],
                                                    func: Box::new(Expr::Path(ExprPath {
                                                        attrs: vec![],
                                                        qself: Some(QSelf {
                                                            lt_token: <Token![<]>::default(),
                                                            ty: Box::new(tuple_type),
                                                            position: 3,
                                                            as_token: Some(<Token![as]>::default()),
                                                            gt_token: <Token![>]>::default(),
                                                        }),
                                                        path: Path {
                                                            leading_colon: Some(
                                                                <Token![::]>::default(),
                                                            ),
                                                            segments: [
                                                                seg(id("pbt")),
                                                                seg(id("conjure")),
                                                                seg(id("Conjure")),
                                                                seg(id("conjure")),
                                                            ]
                                                            .into_iter()
                                                            .collect(),
                                                        },
                                                    })),
                                                    paren_token: Paren::default(),
                                                    args: iter::once(expr_of_str("seed")).collect(),
                                                })),
                                                question_token: <Token![?]>::default(),
                                            })),
                                            diverge: None,
                                        }),
                                        semi_token: <Token![;]>::default(),
                                    }),
                                    Stmt::Expr(
                                        Expr::Call(ExprCall {
                                            attrs: vec![],
                                            func: Box::new(expr_of_str("Ok")),
                                            paren_token: Paren::default(),
                                            args: iter::once(instantiate_variant(variant))
                                                .collect(),
                                        }),
                                        None,
                                    ),
                                ],
                            },
                        })),
                    })),
                    as_token: <Token![as]>::default(),
                    ty: Box::new(fn_type.clone()),
                }),
                */
                Expr::Cast(ExprCast {
                    attrs: vec![],
                    expr: Box::new(Expr::Closure(ExprClosure {
                        attrs: vec![],
                        lifetimes: None,
                        constness: None,
                        movability: None,
                        asyncness: None,
                        capture: None,
                        or1_token: <Token![|]>::default(),
                        inputs: iter::once(Pat::Type(PatType {
                            attrs: vec![],
                            pat: Box::new(
                            Pat::Ident(PatIdent { attrs: vec![], by_ref: None, mutability: Some(<Token![mut]>::default()), ident: id("seed"), subpat: None } )
                            ),
                            colon_token: <Token![:]>::default(),
                            ty: Box::new(seed_type.clone()),
                        }))
                        .collect(),
                        or2_token: <Token![|]>::default(),
                        output: ReturnType::Type(
                            <Token![->]>::default(),
                            Box::new(Type::Path(TypePath {
                                qself: None,
                                path: path_of_str("Self"),
                            })),
                        ),
                        body: Box::new(Expr::Block(ExprBlock {
                            attrs: vec![],
                            label: None,
                            block: Block {
                                brace_token: Brace::default(),
                                stmts: vec![
                                    Stmt::Local(Local {
                                        attrs: vec![],
                                        let_token: <Token![let]>::default(),
                                        pat: tuple_pat,
                                        init: Some(LocalInit {
                                            eq_token: <Token![=]>::default(),
                                            expr: Box::new(Expr::Unsafe(ExprUnsafe {
                                                attrs: vec![],
                                                unsafe_token: <Token![unsafe]>::default(),
                                                block: Block {
                                                    brace_token: Brace::default(),
                                                    stmts: vec![Stmt::Expr(
                                                        Expr::MethodCall(ExprMethodCall {
                                                            attrs: vec![],
                                                            receiver: Box::new(
                                                                Expr::Call(ExprCall {
                                                                    attrs: vec![],
                                                                    func: Box::new(Expr::Path(ExprPath {
                                                                        attrs: vec![],
                                                                        qself: Some(QSelf {
                                                                            lt_token: <Token![<]>::default(
                                                                            ),
                                                                            ty: Box::new(tuple_type),
                                                                            position: 3,
                                                                            as_token: Some(
                                                                                <Token![as]>::default(),
                                                                            ),
                                                                            gt_token: <Token![>]>::default(
                                                                            ),
                                                                        }),
                                                                        path: Path {
                                                                            leading_colon: Some(
                                                                                <Token![::]>::default(),
                                                                            ),
                                                                            segments: [
                                                                                seg(id("pbt")),
                                                                                seg(id("conjure")),
                                                                                seg(id("Conjure")),
                                                                                seg(id("conjure")),
                                                                            ]
                                                                            .into_iter()
                                                                            .collect(),
                                                                        },
                                                                    })),
                                                                    paren_token: Paren::default(),
                                                                    args: iter::once(expr_of_str("seed"))
                                                                        .collect(),
                                                                }),
                                                            ),
                                                            dot_token: <Token![.]>::default(),
                                                            method: id("unwrap_unchecked"),
                                                            turbofish: None,
                                                            paren_token: Paren::default(),
                                                            args: Punctuated::new(),
                                                        }),
                                                        None,
                                                    )],
                                                },
                                            })),
                                            diverge: None,
                                        }),
                                        semi_token: <Token![;]>::default(),
                                    }),
                                    Stmt::Expr(
                                        // Expr::Call(ExprCall {
                                        //     attrs: vec![],
                                        //     func: Box::new(expr_of_str("Ok")),
                                        //     paren_token: Paren::default(),
                                        //     args: iter::once(
                                                instantiate_variant(variant),
                                        //     )
                                        //         .collect(),
                                        // }),
                                        None,
                                    ),
                                ],
                            },
                        })),
                    })),
                    as_token: <Token![as]>::default(),
                    ty: Box::new(fn_type.clone()),
                }),
            ]
            .into_iter()
            .collect(),
        })
    };
    let iterator = Expr::MethodCall(ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(Expr::Array(ExprArray {
            attrs: vec![],
            bracket_token: Bracket::default(),
            elems: item.variants.iter().map(element_per_variant).collect(),
        })),
        dot_token: <Token![.]>::default(),
        method: id("into_iter"),
        turbofish: None,
        paren_token: Paren::default(),
        args: Punctuated::new(),
    });
    /*
    let filtered_n_mapped = Expr::MethodCall(ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(iterator),
        dot_token: <Token![.]>::default(),
        method: id("filter_map"),
        turbofish: None,
        paren_token: Paren::default(),
        args: iter::once(Expr::Closure(ExprClosure {
            attrs: vec![],
            lifetimes: None,
            constness: None,
            movability: None,
            asyncness: None,
            capture: None,
            or1_token: <Token![|]>::default(),
            inputs: iter::once(Pat::Tuple(PatTuple {
                attrs: vec![],
                paren_token: Paren::default(),
                elems: [
                    Pat::Ident(PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: id("size"),
                        subpat: None,
                    }),
                    Pat::Ident(PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: id("f"),
                        subpat: None,
                    }),
                ]
                .into_iter()
                .collect(),
            }))
            .collect(),
            or2_token: <Token![|]>::default(),
            output: ReturnType::Default,
            body: Box::new(Expr::MethodCall(ExprMethodCall {
                attrs: vec![],
                receiver: Box::new(Expr::Macro(ExprMacro {
                    attrs: vec![],
                    mac: Macro {
                        path: path_of_str("matches"),
                        bang_token: <Token![!]>::default(),
                        delimiter: MacroDelimiter::Paren(Paren::default()),
                        tokens: quote! {
                            size,
                            ::pbt::count::Cardinality::Infinite,
                        },
                    },
                })),
                dot_token: <Token![.]>::default(),
                method: id("then_some"),
                turbofish: None,
                paren_token: Paren::default(),
                args: iter::once(expr_of_str("f")).collect(),
            })),
        }))
        .collect(),
    });
    let vector = Expr::MethodCall(ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(iterator),
        dot_token: <Token![.]>::default(),
        method: id("collect"),
        turbofish: None,
        paren_token: Paren::default(),
        args: Punctuated::new(),
    });
    Block {
        brace_token: Brace::default(),
        stmts: vec![Stmt::Expr(vector, None)],
    }
    */
    Block {
        brace_token: Brace::default(),
        stmts: vec![Stmt::Expr(iterator, None)],
    }
}

#[inline]
fn variants_body_for_struct() -> Block {
    Block {
        brace_token: Brace::default(),
        stmts: vec![
            // Stmt::Macro(StmtMacro {
            //     attrs: vec![],
            //     mac: Macro {
            //         path: path_of_str("vec"),
            //         bang_token: <Token![!]>::default(),
            //         delimiter: MacroDelimiter::Bracket(Bracket::default()),
            //         tokens: quote! {},
            //     },
            //     semi_token: None,
            // }),
            /*
            Stmt::Expr(
                Expr::Call(ExprCall {
                    attrs: vec![],
                    func: Box::new(Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: Path {
                            leading_colon: Some(<Token![::]>::default()),
                            segments: [
                                seg(id("core")),
                                seg(id("iter")),
                                PathSegment {
                                    ident: id("empty"),
                                    arguments: PathArguments::AngleBracketed(
                                        AngleBracketedGenericArguments {
                                            colon2_token: Some(<Token![::]>::default()),
                                            lt_token: <Token![<]>::default(),
                                            args: iter::once(GenericArgument::Type(Type::BareFn(
                                                TypeBareFn {
                                                    lifetimes: None,
                                                    unsafety: None,
                                                    abi: None,
                                                    fn_token: <Token![fn]>::default(),
                                                    paren_token: Paren::default(),
                                                    inputs: iter::once(BareFnArg {
                                                        attrs: vec![],
                                                        name: None,
                                                        ty: Type::Path(TypePath {
                                                            qself: None,
                                                            path: Path {
                                                                leading_colon: Some(
                                                                    <Token![::]>::default(),
                                                                ),
                                                                segments: [
                                                                    seg(id("pbt")),
                                                                    seg(id("conjure")),
                                                                    seg(id("Seed")),
                                                                ]
                                                                .into_iter()
                                                                .collect(),
                                                            },
                                                        }),
                                                    })
                                                    .collect(),
                                                    variadic: None,
                                                    output: ReturnType::Type(
                                                        <Token![->]>::default(),
                                                        Box::new(Type::Path(TypePath {
                                                            qself: None,
                                                            path: path_of_str("Self"),
                                                        })),
                                                    ),
                                                },
                                            )))
                                            .collect(),
                                            gt_token: <Token![>]>::default(),
                                        },
                                    ),
                                },
                            ]
                            .into_iter()
                            .collect(),
                        },
                    })),
                    paren_token: Paren::default(),
                    args: Punctuated::new(),
                }),
                None,
            ),
            */
            Stmt::Expr(
                Expr::Call(ExprCall {
                    attrs: vec![],
                    func: Box::new(Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: Path {
                            leading_colon: Some(<Token![::]>::default()),
                            segments: [seg(id("core")), seg(id("iter")), seg(id("once"))]
                                .into_iter()
                                .collect(),
                        },
                    })),
                    paren_token: Paren::default(),
                    args: iter::once(Expr::Tuple(ExprTuple {
                        attrs: vec![],
                        paren_token: Paren::default(),
                        elems: [
                            Expr::Path(ExprPath {
                                attrs: vec![],
                                qself: Some(QSelf {
                                    lt_token: <Token![<]>::default(),
                                    ty: Box::new(Type::Path(TypePath {
                                        qself: None,
                                        path: path_of_str("Self"),
                                    })),
                                    position: 3,
                                    as_token: Some(<Token![as]>::default()),
                                    gt_token: <Token![>]>::default(),
                                }),
                                path: Path {
                                    leading_colon: Some(<Token![::]>::default()),
                                    segments: [
                                        seg(id("pbt")),
                                        seg(id("count")),
                                        seg(id("Count")),
                                        seg(id("CARDINALITY")),
                                    ]
                                    .into_iter()
                                    .collect(),
                                },
                            }),
                            // (|seed| unsafe { Self::conjure(seed).unwrap_unchecked() }) as fn(_) -> _,
                            Expr::Cast(ExprCast {
                                attrs: vec![],
                                expr: Box::new(Expr::Closure(ExprClosure {
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
                                        ident: id("seed"),
                                        subpat: None,
                                    }))
                                    .collect(),
                                    or2_token: <Token![|]>::default(),
                                    output: ReturnType::Default,
                                    body: Box::new(Expr::Unsafe(ExprUnsafe {
                                        attrs: vec![],
                                        unsafe_token: <Token![unsafe]>::default(),
                                        block: Block {
                                            brace_token: Brace::default(),
                                            stmts: vec![Stmt::Expr(
                                                Expr::MethodCall(ExprMethodCall {
                                                    attrs: vec![],
                                                    receiver: Box::new(Expr::Call(ExprCall {
                                                        attrs: vec![],
                                                        func: Box::new(Expr::Path(ExprPath {
                                                            attrs: vec![],
                                                            qself: Some(QSelf {
                                                                lt_token: <Token![<]>::default(),
                                                                ty: Box::new(Type::Path(
                                                                    TypePath {
                                                                        qself: None,
                                                                        path: path_of_str("Self"),
                                                                    },
                                                                )),
                                                                position: 3,
                                                                as_token: None,
                                                                gt_token: <Token![>]>::default(),
                                                            }),
                                                            path: Path {
                                                                leading_colon: Some(
                                                                    <Token![::]>::default(),
                                                                ),
                                                                segments: [
                                                                    seg(id("pbt")),
                                                                    seg(id("conjure")),
                                                                    seg(id("Conjure")),
                                                                    seg(id("conjure")),
                                                                ]
                                                                .into_iter()
                                                                .collect(),
                                                            },
                                                        })),
                                                        paren_token: Paren::default(),
                                                        args: iter::once(expr_of_str("seed"))
                                                            .collect(),
                                                    })),
                                                    dot_token: <Token![.]>::default(),
                                                    method: id("unwrap_unchecked"),
                                                    turbofish: None,
                                                    paren_token: Paren::default(),
                                                    args: Punctuated::new(),
                                                }),
                                                None,
                                            )],
                                        },
                                    })),
                                })),
                                as_token: <Token![as]>::default(),
                                ty: Box::new(Type::BareFn(TypeBareFn {
                                    lifetimes: None,
                                    unsafety: None,
                                    abi: None,
                                    fn_token: <Token![fn]>::default(),
                                    paren_token: Paren::default(),
                                    inputs: iter::once(BareFnArg {
                                        attrs: vec![],
                                        name: None,
                                        ty: Type::Infer(TypeInfer {
                                            underscore_token: <Token![_]>::default(),
                                        }),
                                    })
                                    .collect(),
                                    variadic: None,
                                    output: ReturnType::Type(
                                        <Token![->]>::default(),
                                        Box::new(Type::Infer(TypeInfer {
                                            underscore_token: <Token![_]>::default(),
                                        })),
                                    ),
                                })),
                            }),
                        ]
                        .into_iter()
                        .collect(),
                    }))
                    .collect(),
                }),
                None,
            ),
        ],
    }
}

#[inline]
fn conjure_body_for_enum() -> Block {
    let should_recurse = Expr::MethodCall(ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(expr_of_str("seed")),
        dot_token: <Token![.]>::default(),
        method: id("should_recurse"),
        turbofish: None,
        paren_token: Paren::default(),
        args: Punctuated::new(),
    });
    let let_variants = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::Struct(PatStruct {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: Some(<Token![::]>::default()),
                segments: [seg(id("pbt")), seg(id("conjure")), seg(id("Variants"))]
                    .into_iter()
                    .collect(),
            },
            brace_token: Brace::default(),
            fields: [
                FieldPat {
                    attrs: vec![],
                    member: Member::Named(id("internal_nodes")),
                    colon_token: None,
                    pat: Box::new(Pat::Ident(PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: id("internal_nodes"),
                        subpat: None,
                    })),
                },
                FieldPat {
                    attrs: vec![],
                    member: Member::Named(id("leaves")),
                    colon_token: None,
                    pat: Box::new(Pat::Ident(PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: id("leaves"),
                        subpat: None,
                    })),
                },
            ]
            .into_iter()
            .collect(),
            rest: None,
        }),
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::Call(ExprCall {
                attrs: vec![],
                func: Box::new(Expr::Path(ExprPath {
                    attrs: vec![],
                    qself: None,
                    path: Path {
                        leading_colon: Some(<Token![::]>::default()),
                        segments: [
                            seg(id("pbt")),
                            seg(id("conjure")),
                            PathSegment {
                                ident: id("variants"),
                                arguments: PathArguments::AngleBracketed(
                                    AngleBracketedGenericArguments {
                                        colon2_token: Some(<Token![::]>::default()),
                                        lt_token: <Token![<]>::default(),
                                        args: iter::once(GenericArgument::Type(Type::Path(
                                            TypePath {
                                                qself: None,
                                                path: path_of_str("Self"),
                                            },
                                        )))
                                        .collect(),
                                        gt_token: <Token![>]>::default(),
                                    },
                                ),
                            },
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
    };
    let let_nz_leaves = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::Ident(PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: None,
            ident: id("nz"),
            subpat: None,
        }),
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::Try(ExprTry {
                attrs: vec![],
                expr: Box::new(Expr::MethodCall(ExprMethodCall {
                    attrs: vec![],
                    receiver: Box::new(Expr::Call(ExprCall {
                        attrs: vec![],
                        func: Box::new(Expr::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: Path {
                                leading_colon: Some(<Token![::]>::default()),
                                segments: [
                                    seg(id("core")),
                                    seg(id("num")),
                                    seg(id("NonZero")),
                                    seg(id("new")),
                                ]
                                .into_iter()
                                .collect(),
                            },
                        })),
                        paren_token: Paren::default(),
                        args: iter::once(Expr::MethodCall(ExprMethodCall {
                            attrs: vec![],
                            receiver: Box::new(expr_of_str("leaves")),
                            dot_token: <Token![.]>::default(),
                            method: id("len"),
                            turbofish: None,
                            paren_token: Paren::default(),
                            args: Punctuated::new(),
                        }))
                        .collect(),
                    })),
                    dot_token: <Token![.]>::default(),
                    method: id("ok_or"),
                    turbofish: None,
                    paren_token: Paren::default(),
                    args: iter::once(Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: Path {
                            leading_colon: Some(<Token![::]>::default()),
                            segments: [
                                seg(id("pbt")),
                                seg(id("conjure")),
                                seg(id("Uninstantiable")),
                            ]
                            .into_iter()
                            .collect(),
                        },
                    }))
                    .collect(),
                })),
                question_token: <Token![?]>::default(),
            })),
            diverge: None,
        }),
        semi_token: <Token![;]>::default(),
    };
    let let_i = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::Ident(PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: None,
            ident: id("i"),
            subpat: None,
        }),
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::Binary(ExprBinary {
                attrs: vec![],
                left: Box::new(Expr::Cast(ExprCast {
                    attrs: vec![],
                    expr: Box::new(Expr::MethodCall(ExprMethodCall {
                        attrs: vec![],
                        receiver: Box::new(expr_of_str("seed")),
                        dot_token: <Token![.]>::default(),
                        method: id("prng"),
                        turbofish: None,
                        paren_token: Paren::default(),
                        args: Punctuated::new(),
                    })),
                    as_token: <Token![as]>::default(),
                    ty: Box::new(Type::Path(TypePath {
                        qself: None,
                        path: path_of_str("usize"),
                    })),
                })),
                op: BinOp::Rem(<Token![%]>::default()),
                right: Box::new(expr_of_str("nz")),
            })),
            diverge: None,
        }),
        semi_token: <Token![;]>::default(),
    };
    let let_nz = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::TupleStruct(PatTupleStruct {
            attrs: vec![],
            qself: None,
            path: path_of_str("Some"),
            paren_token: Paren::default(),
            elems: iter::once(Pat::Ident(PatIdent {
                attrs: vec![],
                by_ref: None,
                mutability: None,
                ident: id("nz"),
                subpat: None,
            }))
            .collect(),
        }),
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::Call(ExprCall {
                attrs: vec![],
                func: Box::new(Expr::Path(ExprPath {
                    attrs: vec![],
                    qself: None,
                    path: Path {
                        leading_colon: Some(<Token![::]>::default()),
                        segments: [
                            seg(id("core")),
                            seg(id("num")),
                            seg(id("NonZero")),
                            seg(id("new")),
                        ]
                        .into_iter()
                        .collect(),
                    },
                })),
                paren_token: Paren::default(),
                args: iter::once(Expr::MethodCall(ExprMethodCall {
                    attrs: vec![],
                    receiver: Box::new(expr_of_str("internal_nodes")),
                    dot_token: <Token![.]>::default(),
                    method: id("len"),
                    turbofish: None,
                    paren_token: Paren::default(),
                    args: Punctuated::new(),
                }))
                .collect(),
            })),
            diverge: Some((
                <Token![else]>::default(),
                Box::new(Expr::Block(ExprBlock {
                    attrs: vec![],
                    label: None,
                    block: Block {
                        brace_token: Brace::default(),
                        stmts: vec![
                            Stmt::Local(let_nz_leaves),
                            Stmt::Local(let_i.clone()),
                            Stmt::Expr(
                                Expr::Return(ExprReturn {
                                    attrs: vec![],
                                    return_token: <Token![return]>::default(),
                                    expr: Some(Box::new(Expr::Call(ExprCall {
                                        attrs: vec![],
                                        func: Box::new(expr_of_str("Ok")),
                                        paren_token: Paren::default(),
                                        args: iter::once(Expr::Call(ExprCall {
                                            attrs: vec![],
                                            func: Box::new(Expr::Unsafe(ExprUnsafe {
                                                attrs: vec![],
                                                unsafe_token: <Token![unsafe]>::default(),
                                                block: Block {
                                                    brace_token: Brace::default(),
                                                    stmts: vec![Stmt::Expr(
                                                        Expr::MethodCall(ExprMethodCall {
                                                            attrs: vec![],
                                                            receiver: Box::new(expr_of_str(
                                                                "leaves",
                                                            )),
                                                            dot_token: <Token![.]>::default(),
                                                            method: id("get_unchecked"),
                                                            turbofish: None,
                                                            paren_token: Paren::default(),
                                                            args: iter::once(expr_of_str("i"))
                                                                .collect(),
                                                        }),
                                                        None,
                                                    )],
                                                },
                                            })),
                                            paren_token: Paren::default(),
                                            args: iter::once(expr_of_str("seed")).collect(),
                                        }))
                                        .collect(),
                                    }))),
                                }),
                                Some(<Token![;]>::default()),
                            ),
                        ],
                    },
                })),
            )),
        }),
        semi_token: <Token![;]>::default(),
    };
    let expr = Expr::If(ExprIf {
        attrs: vec![],
        if_token: <Token![if]>::default(),
        cond: Box::new(should_recurse),
        then_branch: Block {
            brace_token: Brace::default(),
            stmts: vec![
                Stmt::Local(let_variants),
                Stmt::Local(let_nz),
                Stmt::Local(let_i),
                Stmt::Expr(
                    Expr::Call(ExprCall {
                        attrs: vec![],
                        func: Box::new(expr_of_str("Ok")),
                        paren_token: Paren::default(),
                        args: iter::once(Expr::Call(ExprCall {
                            attrs: vec![],
                            func: Box::new(Expr::Unsafe(ExprUnsafe {
                                attrs: vec![],
                                unsafe_token: <Token![unsafe]>::default(),
                                block: Block {
                                    brace_token: Brace::default(),
                                    stmts: vec![Stmt::Expr(
                                        Expr::MethodCall(ExprMethodCall {
                                            attrs: vec![],
                                            receiver: Box::new(expr_of_str("internal_nodes")),
                                            dot_token: <Token![.]>::default(),
                                            method: id("get_unchecked"),
                                            turbofish: None,
                                            paren_token: Paren::default(),
                                            args: iter::once(expr_of_str("i")).collect(),
                                        }),
                                        None,
                                    )],
                                },
                            })),
                            paren_token: Paren::default(),
                            args: iter::once(expr_of_str("seed")).collect(),
                        }))
                        .collect(),
                    }),
                    None,
                ),
            ],
        },
        else_branch: Some((
            <Token![else]>::default(),
            Box::new(Expr::Call(ExprCall {
                attrs: vec![],
                func: Box::new(Expr::Path(ExprPath {
                    attrs: vec![],
                    qself: Some(QSelf {
                        lt_token: <Token![<]>::default(),
                        ty: Box::new(Type::Path(TypePath {
                            qself: None,
                            path: path_of_str("Self"),
                        })),
                        position: 3,
                        as_token: Some(<Token![as]>::default()),
                        gt_token: <Token![>]>::default(),
                    }),
                    path: Path {
                        leading_colon: Some(<Token![::]>::default()),
                        segments: [
                            seg(id("pbt")),
                            seg(id("conjure")),
                            seg(id("Conjure")),
                            seg(id("leaf")),
                        ]
                        .into_iter()
                        .collect(),
                    },
                })),
                paren_token: Paren::default(),
                args: iter::once(expr_of_str("seed")).collect(),
            })),
        )),
    });

    Block {
        brace_token: Brace::default(),
        stmts: vec![Stmt::Expr(expr, None)],
    }
}

#[inline]
fn conjure_body_for_struct(item: &ItemStruct) -> Block {
    /*
    #[inline]
    fn force_seed_id(maybe_id: Option<&Ident>, index: usize) -> Ident {
        id(&maybe_id.map_or_else(|| format!("_{index}_seed"), |id| format!("{id}_seed")))
    }

    let field_seeds_let = Stmt::Local(Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::Slice(PatSlice {
            attrs: vec![],
            bracket_token: Bracket::default(),
            elems: item
                .fields
                .iter()
                .enumerate()
                .map(|(i, field)| {
                    Pat::Ident(PatIdent {
                        attrs: vec![],
                        by_ref: None,
                        mutability: None,
                        ident: force_seed_id(field.ident.as_ref(), i),
                        subpat: None,
                    })
                })
                .collect(),
        }),
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::MethodCall(ExprMethodCall {
                attrs: vec![],
                receiver: Box::new(expr_of_str("seed")),
                dot_token: <Token![.]>::default(),
                method: id("split"),
                turbofish: None,
                paren_token: Paren::default(),
                args: Punctuated::new(),
            })),
            diverge: None,
        }),
        semi_token: <Token![;]>::default(),
    });
    let field_lets = item.fields.iter().enumerate().map(|(i, field)| {
        Stmt::Local(Local {
            attrs: vec![],
            let_token: <Token![let]>::default(),
            pat: Pat::Ident(PatIdent {
                attrs: vec![],
                by_ref: None,
                mutability: None,
                ident: force_id(field.ident.as_ref(), i),
                subpat: None,
            }),
            init: Some(LocalInit {
                eq_token: <Token![=]>::default(),
                expr: Box::new(Expr::Call(ExprCall {
                    attrs: vec![],
                    func: Box::new(Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: Some(QSelf {
                            lt_token: <Token![<]>::default(),
                            ty: Box::new(field.ty.clone()),
                            position: 3,
                            as_token: Some(<Token![as]>::default()),
                            gt_token: <Token![>]>::default(),
                        }),
                        path: Path {
                            leading_colon: Some(<Token![::]>::default()),
                            segments: [
                                seg(id("pbt")),
                                seg(id("conjure")),
                                seg(id("Conjure")),
                                seg(id("variants")),
                            ]
                            .into_iter()
                            .collect(),
                        },
                    })),
                    paren_token: Paren::default(),
                    args: iter::once(Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: path_of_id(force_seed_id(field.ident.as_ref(), i)),
                    }))
                    .collect(),
                })),
                diverge: None,
            }),
            semi_token: <Token![;]>::default(),
        })
    });

    Block {
        brace_token: Brace::default(),
        stmts: iter::once(field_seeds_let)
            .chain(field_lets)
            .chain(iter::once(Stmt::Expr(
                instantiate_fields(path_of_str("Self"), &item.fields),
                None,
            )))
            .collect(),
    }
    */

    let (tuple_type, tuple_pat) = tuples_of_fields(&item.fields);
    Block {
        brace_token: Brace::default(),
        stmts: vec![
            Stmt::Local(Local {
                attrs: vec![],
                let_token: <Token![let]>::default(),
                pat: tuple_pat,
                init: Some(LocalInit {
                    eq_token: <Token![=]>::default(),
                    expr: Box::new(Expr::Try(ExprTry {
                        attrs: vec![],
                        expr: Box::new(Expr::Call(ExprCall {
                            attrs: vec![],
                            func: Box::new(Expr::Path(ExprPath {
                                attrs: vec![],
                                qself: Some(QSelf {
                                    lt_token: <Token![<]>::default(),
                                    ty: Box::new(tuple_type),
                                    position: 3,
                                    as_token: Some(<Token![as]>::default()),
                                    gt_token: <Token![>]>::default(),
                                }),
                                path: Path {
                                    leading_colon: Some(<Token![::]>::default()),
                                    segments: [
                                        seg(id("pbt")),
                                        seg(id("conjure")),
                                        seg(id("Conjure")),
                                        seg(id("conjure")),
                                    ]
                                    .into_iter()
                                    .collect(),
                                },
                            })),
                            paren_token: Paren::default(),
                            args: iter::once(expr_of_str("seed")).collect(),
                        })),
                        question_token: <Token![?]>::default(),
                    })),
                    diverge: None,
                }),
                semi_token: <Token![;]>::default(),
            }),
            Stmt::Expr(
                Expr::Call(ExprCall {
                    attrs: vec![],
                    func: Box::new(expr_of_str("Ok")),
                    paren_token: Paren::default(),
                    args: iter::once(instantiate_fields(path_of_str("Self"), &item.fields))
                        .collect(),
                }),
                None,
            ),
        ],
    }
}

#[inline]
fn corners_of_fields(path: Path, fields: &Fields) -> Expr {
    let (tuple_type, tuple_pat) = tuples_of_fields(fields);
    let corners_of_tuple = Expr::Call(ExprCall {
        attrs: vec![],
        func: Box::new(Expr::Path(ExprPath {
            attrs: vec![],
            qself: Some(QSelf {
                lt_token: <Token![<]>::default(),
                ty: Box::new(tuple_type),
                position: 3,
                as_token: Some(<Token![as]>::default()),
                gt_token: <Token![>]>::default(),
            }),
            path: Path {
                leading_colon: Some(<Token![::]>::default()),
                segments: [
                    seg(id("pbt")),
                    seg(id("conjure")),
                    seg(id("Conjure")),
                    seg(id("corners")),
                ]
                .into_iter()
                .collect(),
            },
        })),
        paren_token: Paren::default(),
        args: Punctuated::new(),
    });
    let mapping = Expr::Closure(ExprClosure {
        attrs: vec![],
        lifetimes: None,
        constness: None,
        movability: None,
        asyncness: None,
        capture: None,
        or1_token: <Token![|]>::default(),
        inputs: iter::once(tuple_pat).collect(),
        or2_token: <Token![|]>::default(),
        output: ReturnType::Default,
        body: Box::new(instantiate_fields(path, fields)),
    });
    Expr::MethodCall(ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(corners_of_tuple),
        dot_token: <Token![.]>::default(),
        method: id("map"),
        turbofish: None,
        paren_token: Paren::default(),
        args: iter::once(mapping).collect(),
    })
}

#[inline]
fn corners_of_variant(variant: &Variant) -> Expr {
    corners_of_fields(
        Path {
            leading_colon: None,
            segments: [seg(id("Self")), seg(variant.ident.clone())]
                .into_iter()
                .collect(),
        },
        &variant.fields,
    )
}

#[inline]
fn corners_body_for_enum(item: &ItemEnum) -> Block {
    let empty = Expr::Call(ExprCall {
        attrs: vec![],
        func: Box::new(Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: Some(<Token![::]>::default()),
                segments: [
                    seg(id("core")),
                    seg(id("iter")),
                    // seg(id("empty")),
                    PathSegment {
                        ident: id("empty"),
                        arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            colon2_token: Some(<Token![::]>::default()),
                            lt_token: <Token![<]>::default(),
                            args: iter::once(GenericArgument::Type(Type::Path(TypePath {
                                qself: None,
                                path: path_of_str("Self"),
                            })))
                            .collect(),
                            gt_token: <Token![>]>::default(),
                        }),
                    },
                ]
                .into_iter()
                .collect(),
            },
        })),
        paren_token: Paren::default(),
        args: Punctuated::new(),
    });
    let iter = item.variants.iter().fold(empty, |acc, variant| {
        Expr::MethodCall(ExprMethodCall {
            attrs: vec![],
            receiver: Box::new(acc),
            dot_token: <Token![.]>::default(),
            method: id("chain"),
            turbofish: None,
            paren_token: Paren::default(),
            args: iter::once(corners_of_variant(variant)).collect(),
        })
    });
    let boxed = Expr::Call(ExprCall {
        attrs: vec![],
        func: Box::new(Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: None,
                segments: [seg(id("Box")), seg(id("new"))].into_iter().collect(),
            },
        })),
        paren_token: Paren::default(),
        args: iter::once(iter).collect(),
    });
    Block {
        brace_token: Brace::default(),
        stmts: vec![Stmt::Expr(boxed, None)],
    }
}

#[inline]
fn corners_body_for_struct(item: &ItemStruct) -> Block {
    let boxed = Expr::Call(ExprCall {
        attrs: vec![],
        func: Box::new(Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: None,
                segments: [seg(id("Box")), seg(id("new"))].into_iter().collect(),
            },
        })),
        paren_token: Paren::default(),
        args: iter::once(corners_of_fields(path_of_str("Self"), &item.fields)).collect(),
    });
    Block {
        brace_token: Brace::default(),
        stmts: vec![Stmt::Expr(boxed, None)],
    }
}

#[inline]
fn leaf_body_for_enum(_item: &ItemEnum) -> Block {
    let leaves_iter = Expr::Call(ExprCall {
        attrs: vec![],
        func: Box::new(Expr::Path(ExprPath {
            attrs: vec![],
            qself: None,
            path: Path {
                leading_colon: Some(<Token![::]>::default()),
                segments: [
                    seg(id("pbt")),
                    seg(id("conjure")),
                    PathSegment {
                        ident: id("leaves"),
                        arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                            colon2_token: Some(<Token![::]>::default()),
                            lt_token: <Token![<]>::default(),
                            args: iter::once(GenericArgument::Type(Type::Path(TypePath {
                                qself: None,
                                path: path_of_str("Self"),
                            })))
                            .collect(),
                            gt_token: <Token![>]>::default(),
                        }),
                    },
                ]
                .into_iter()
                .collect(),
            },
        })),
        paren_token: Paren::default(),
        args: Punctuated::new(),
    });
    let let_leaves = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::Ident(PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: None,
            ident: id("leaves"),
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
                        leading_colon: None,
                        segments: [seg(id("Vec")), seg(id("from_iter"))].into_iter().collect(),
                    },
                })),
                paren_token: Paren::default(),
                args: iter::once(leaves_iter).collect(),
            })),
            diverge: None,
        }),
        semi_token: <Token![;]>::default(),
    };

    let let_nz = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::Ident(PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: None,
            ident: id("nz"),
            subpat: None,
        }),
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::Try(ExprTry {
                attrs: vec![],
                expr: Box::new(Expr::MethodCall(ExprMethodCall {
                    attrs: vec![],
                    receiver: Box::new(Expr::Call(ExprCall {
                        attrs: vec![],
                        func: Box::new(Expr::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: Path {
                                leading_colon: Some(<Token![::]>::default()),
                                segments: [
                                    seg(id("core")),
                                    seg(id("num")),
                                    seg(id("NonZero")),
                                    seg(id("new")),
                                ]
                                .into_iter()
                                .collect(),
                            },
                        })),
                        paren_token: Paren::default(),
                        args: iter::once(Expr::MethodCall(ExprMethodCall {
                            attrs: vec![],
                            receiver: Box::new(expr_of_str("leaves")),
                            dot_token: <Token![.]>::default(),
                            method: id("len"),
                            turbofish: None,
                            paren_token: Paren::default(),
                            args: Punctuated::new(),
                        }))
                        .collect(),
                    })),
                    dot_token: <Token![.]>::default(),
                    method: id("ok_or"),
                    turbofish: None,
                    paren_token: Paren::default(),
                    args: iter::once(Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path: Path {
                            leading_colon: Some(<Token![::]>::default()),
                            segments: [
                                seg(id("pbt")),
                                seg(id("conjure")),
                                seg(id("Uninstantiable")),
                            ]
                            .into_iter()
                            .collect(),
                        },
                    }))
                    .collect(),
                })),
                question_token: <Token![?]>::default(),
            })),
            diverge: None,
        }),
        semi_token: <Token![;]>::default(),
    };
    let let_i = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: Pat::Ident(PatIdent {
            attrs: vec![],
            by_ref: None,
            mutability: None,
            ident: id("i"),
            subpat: None,
        }),
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::Binary(ExprBinary {
                attrs: vec![],
                left: Box::new(Expr::Cast(ExprCast {
                    attrs: vec![],
                    expr: Box::new(Expr::MethodCall(ExprMethodCall {
                        attrs: vec![],
                        receiver: Box::new(expr_of_str("seed")),
                        dot_token: <Token![.]>::default(),
                        method: id("prng"),
                        turbofish: None,
                        paren_token: Paren::default(),
                        args: Punctuated::new(),
                    })),
                    as_token: <Token![as]>::default(),
                    ty: Box::new(Type::Path(TypePath {
                        qself: None,
                        path: path_of_str("usize"),
                    })),
                })),
                op: BinOp::Rem(<Token![%]>::default()),
                right: Box::new(expr_of_str("nz")),
            })),
            diverge: None,
        }),
        semi_token: <Token![;]>::default(),
    };

    Block {
        brace_token: Brace::default(),
        stmts: vec![
            Stmt::Local(let_leaves),
            Stmt::Local(let_nz),
            Stmt::Local(let_i),
            Stmt::Expr(
                Expr::Call(ExprCall {
                    attrs: vec![],
                    func: Box::new(expr_of_str("Ok")),
                    paren_token: Paren::default(),
                    args: iter::once(Expr::Call(ExprCall {
                        attrs: vec![],
                        func: Box::new(Expr::Unsafe(ExprUnsafe {
                            attrs: vec![],
                            unsafe_token: <Token![unsafe]>::default(),
                            block: Block {
                                brace_token: Brace::default(),
                                stmts: vec![Stmt::Expr(
                                    Expr::MethodCall(ExprMethodCall {
                                        attrs: vec![],
                                        receiver: Box::new(expr_of_str("leaves")),
                                        dot_token: <Token![.]>::default(),
                                        method: id("get_unchecked"),
                                        turbofish: None,
                                        paren_token: Paren::default(),
                                        args: iter::once(expr_of_str("i")).collect(),
                                    }),
                                    None,
                                )],
                            },
                        })),
                        paren_token: Paren::default(),
                        args: iter::once(expr_of_str("seed")).collect(),
                    }))
                    .collect(),
                }),
                None,
            ),
        ],
    }
}

#[inline]
fn leaf_body_for_struct(item: &ItemStruct) -> Block {
    let (tuple_type, tuple_pat) = tuples_of_fields(&item.fields);
    Block {
        brace_token: Brace::default(),
        stmts: vec![
            Stmt::Local(Local {
                attrs: vec![],
                let_token: <Token![let]>::default(),
                pat: tuple_pat,
                init: Some(LocalInit {
                    eq_token: <Token![=]>::default(),
                    expr: Box::new(Expr::Try(ExprTry {
                        attrs: vec![],
                        expr: Box::new(Expr::Call(ExprCall {
                            attrs: vec![],
                            func: Box::new(Expr::Path(ExprPath {
                                attrs: vec![],
                                qself: Some(QSelf {
                                    lt_token: <Token![<]>::default(),
                                    ty: Box::new(tuple_type),
                                    position: 3,
                                    as_token: Some(<Token![as]>::default()),
                                    gt_token: <Token![>]>::default(),
                                }),
                                path: Path {
                                    leading_colon: Some(<Token![::]>::default()),
                                    segments: [
                                        seg(id("pbt")),
                                        seg(id("conjure")),
                                        seg(id("Conjure")),
                                        seg(id("leaf")),
                                    ]
                                    .into_iter()
                                    .collect(),
                                },
                            })),
                            paren_token: Paren::default(),
                            args: iter::once(expr_of_str("seed")).collect(),
                        })),
                        question_token: <Token![?]>::default(),
                    })),
                    diverge: None,
                }),
                semi_token: <Token![;]>::default(),
            }),
            Stmt::Expr(
                Expr::Call(ExprCall {
                    attrs: vec![],
                    func: Box::new(expr_of_str("Ok")),
                    paren_token: Paren::default(),
                    args: iter::once(instantiate_fields(path_of_str("Self"), &item.fields))
                        .collect(),
                }),
                None,
            ),
        ],
    }
}

#[inline]
fn step_fields(path: Path, fields: &Fields) -> Expr {
    let (tuple_type, tuple_pat) = tuples_of_fields(fields);
    let (named, punctuated) = match *fields {
        Fields::Unit => return expr_of_str("None"),
        Fields::Named(ref fields) => (true, &fields.named),
        Fields::Unnamed(ref fields) => (false, &fields.unnamed),
    };
    let clone_path = Path {
        leading_colon: Some(<Token![::]>::default()),
        segments: [
            seg(id("core")),
            seg(id("clone")),
            seg(id("Clone")),
            seg(id("clone")),
        ]
        .into_iter()
        .collect(),
    };
    Expr::MethodCall(ExprMethodCall {
        attrs: vec![],
        receiver: Box::new(Expr::Call(ExprCall {
            attrs: vec![],
            func: Box::new(Expr::Path(ExprPath {
                attrs: vec![],
                qself: Some(QSelf {
                    lt_token: <Token![<]>::default(),
                    ty: Box::new(tuple_type),
                    position: 3,
                    as_token: Some(<Token![as]>::default()),
                    gt_token: <Token![>]>::default(),
                }),
                path: Path {
                    leading_colon: Some(<Token![::]>::default()),
                    segments: [
                        seg(id("pbt")),
                        seg(id("shrink")),
                        seg(id("Shrink")),
                        seg(id("step")),
                    ]
                    .into_iter()
                    .collect(),
                },
            })),
            paren_token: Paren::default(),
            args: [
                Expr::Reference(ExprReference {
                    attrs: vec![],
                    and_token: <Token![&]>::default(),
                    mutability: None,
                    expr: Box::new(Expr::Tuple(ExprTuple {
                        attrs: vec![],
                        paren_token: Paren::default(),
                        elems: punctuated
                            .iter()
                            .enumerate()
                            .map(|(i, field)| {
                                Expr::Call(ExprCall {
                                    attrs: vec![],
                                    func: Box::new(Expr::Path(ExprPath {
                                        attrs: vec![],
                                        qself: Some(QSelf {
                                            lt_token: <Token![<]>::default(),
                                            ty: Box::new(field.ty.clone()),
                                            position: 3,
                                            as_token: Some(<Token![as]>::default()),
                                            gt_token: <Token![>]>::default(),
                                        }),
                                        path: clone_path.clone(),
                                    })),
                                    paren_token: Paren::default(),
                                    args: iter::once(expr_of_id(force_id(field.ident.as_ref(), i)))
                                        .collect(),
                                })
                            })
                            .collect(),
                    })),
                }),
                Expr::Reference(ExprReference {
                    attrs: vec![],
                    and_token: <Token![&]>::default(),
                    mutability: Some(<Token![mut]>::default()),
                    expr: Box::new(Expr::Closure(ExprClosure {
                        attrs: vec![],
                        lifetimes: None,
                        constness: None,
                        movability: None,
                        asyncness: None,
                        capture: None,
                        or1_token: <Token![|]>::default(),
                        inputs: iter::once(tuple_pat.clone()).collect(),
                        or2_token: <Token![|]>::default(),
                        output: ReturnType::Default,
                        // e.g. `ok.step(&mut |ok| property(&Ok(ok.clone()))).map(Ok)`
                        body: Box::new(Expr::Call(ExprCall {
                            attrs: vec![],
                            func: Box::new(expr_of_str("property")),
                            paren_token: Paren::default(),
                            args: iter::once(Expr::Reference(ExprReference {
                                attrs: vec![],
                                and_token: <Token![&]>::default(),
                                mutability: None,
                                expr: Box::new(if named {
                                    Expr::Struct(ExprStruct {
                                        attrs: vec![],
                                        qself: None,
                                        path: path.clone(),
                                        brace_token: Brace::default(),
                                        fields: punctuated
                                            .iter()
                                            .enumerate()
                                            .map(|(i, field)| {
                                                let id = force_id(field.ident.as_ref(), i);
                                                FieldValue {
                                                    attrs: vec![],
                                                    member: Member::Named(id.clone()),
                                                    colon_token: Some(<Token![:]>::default()),
                                                    expr: Expr::Call(ExprCall {
                                                        attrs: vec![],
                                                        func: Box::new(Expr::Path(ExprPath {
                                                            attrs: vec![],
                                                            qself: Some(QSelf {
                                                                lt_token: <Token![<]>::default(),
                                                                ty: Box::new(field.ty.clone()),
                                                                position: 3,
                                                                as_token: Some(
                                                                    <Token![as]>::default(),
                                                                ),
                                                                gt_token: <Token![>]>::default(),
                                                            }),
                                                            path: clone_path.clone(),
                                                        })),
                                                        paren_token: Paren::default(),
                                                        args: iter::once(expr_of_id(id)).collect(),
                                                    }),
                                                }
                                            })
                                            .collect(),
                                        dot2_token: None,
                                        rest: None,
                                    })
                                } else {
                                    Expr::Call(ExprCall {
                                        attrs: vec![],
                                        func: Box::new(Expr::Path(ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path: path.clone(),
                                        })),
                                        paren_token: Paren::default(),
                                        args: punctuated
                                            .iter()
                                            .enumerate()
                                            .map(|(i, field)| {
                                                Expr::Call(ExprCall {
                                                    attrs: vec![],
                                                    func: Box::new(Expr::Path(ExprPath {
                                                        attrs: vec![],
                                                        qself: Some(QSelf {
                                                            lt_token: <Token![<]>::default(),
                                                            ty: Box::new(field.ty.clone()),
                                                            position: 3,
                                                            as_token: Some(<Token![as]>::default()),
                                                            gt_token: <Token![>]>::default(),
                                                        }),
                                                        path: clone_path.clone(),
                                                    })),
                                                    paren_token: Paren::default(),
                                                    args: iter::once(expr_of_id(force_id(
                                                        field.ident.as_ref(),
                                                        i,
                                                    )))
                                                    .collect(),
                                                })
                                            })
                                            .collect(),
                                    })
                                }),
                            }))
                            .collect(),
                        })),
                    })),
                }),
            ]
            .into_iter()
            .collect(),
        })),
        dot_token: <Token![.]>::default(),
        method: id("map"),
        turbofish: None,
        paren_token: Paren::default(),
        args: iter::once(Expr::Closure(ExprClosure {
            attrs: vec![],
            lifetimes: None,
            constness: None,
            movability: None,
            asyncness: None,
            capture: None,
            or1_token: <Token![|]>::default(),
            inputs: iter::once(tuple_pat).collect(),
            or2_token: <Token![|]>::default(),
            output: ReturnType::Default,
            body: Box::new(if named {
                Expr::Struct(ExprStruct {
                    attrs: vec![],
                    qself: None,
                    path,
                    brace_token: Brace::default(),
                    fields: punctuated
                        .iter()
                        .enumerate()
                        .map(|(i, field)| {
                            let id = force_id(field.ident.as_ref(), i);
                            FieldValue {
                                attrs: vec![],
                                member: Member::Named(id.clone()),
                                colon_token: Some(<Token![:]>::default()),
                                expr: expr_of_id(id),
                            }
                        })
                        .collect(),
                    dot2_token: None,
                    rest: None,
                })
            } else {
                Expr::Call(ExprCall {
                    attrs: vec![],
                    func: Box::new(Expr::Path(ExprPath {
                        attrs: vec![],
                        qself: None,
                        path,
                    })),
                    paren_token: Paren::default(),
                    args: punctuated
                        .iter()
                        .enumerate()
                        .map(|(i, field)| expr_of_id(force_id(field.ident.as_ref(), i)))
                        .collect(),
                })
            }),
        }))
        .collect(),
    })
}

#[inline]
fn step_body_for_enum(item: &ItemEnum) -> Block {
    let expr = Expr::Match(ExprMatch {
        attrs: vec![],
        match_token: <Token![match]>::default(),
        expr: Box::new(Expr::Unary(ExprUnary {
            attrs: vec![],
            op: UnOp::Deref(<Token![*]>::default()),
            expr: Box::new(expr_of_str("self")),
        })),
        brace_token: Brace::default(),
        arms: item
            .variants
            .iter()
            .enumerate()
            .map(|(variant_index, variant)| {
                let path = Path {
                    leading_colon: None,
                    segments: [seg(id("Self")), seg(variant.ident.clone())]
                        .into_iter()
                        .collect(),
                };
                Arm {
                    attrs: vec![],
                    pat: match variant.fields {
                        Fields::Unit => Pat::Path(PatPath {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                        }),
                        Fields::Named(ref named) => Pat::Struct(PatStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            brace_token: Brace::default(),
                            fields: named
                                .named
                                .iter()
                                .enumerate()
                                .map(|(i, field)| {
                                    let id = force_id(field.ident.as_ref(), i);
                                    FieldPat {
                                        attrs: vec![],
                                        member: Member::Named(id.clone()),
                                        colon_token: None,
                                        pat: Box::new(Pat::Ident(PatIdent {
                                            attrs: vec![],
                                            by_ref: Some(<Token![ref]>::default()),
                                            mutability: None,
                                            ident: id,
                                            subpat: None,
                                        })),
                                    }
                                })
                                .collect(),
                            rest: None,
                        }),
                        Fields::Unnamed(ref unnamed) => Pat::TupleStruct(PatTupleStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            paren_token: Paren::default(),
                            elems: unnamed
                                .unnamed
                                .iter()
                                .enumerate()
                                .map(|(i, field)| {
                                    Pat::Ident(PatIdent {
                                        attrs: vec![],
                                        by_ref: Some(<Token![ref]>::default()),
                                        mutability: None,
                                        ident: force_id(field.ident.as_ref(), i),
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
                            stmts: item
                                .variants
                                .iter()
                                .take(variant_index)
                                .map(|variant| {
                                    let (tuple_type, tuple_pat) = tuples_of_variant(variant);
                                    let path = Path {
                                        leading_colon: None,
                                        segments: [seg(id("Self")), seg(variant.ident.clone())]
                                            .into_iter()
                                            .collect(),
                                    };
                                    let instantiate = match variant.fields {
                                        Fields::Unit => Expr::Path(ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path,
                                        }),
                                        Fields::Named(ref fields) => Expr::Struct(ExprStruct {
                                            attrs: vec![],
                                            qself: None,
                                            path,
                                            brace_token: Brace::default(),
                                            fields: fields
                                                .named
                                                .iter()
                                                .enumerate()
                                                .map(|(i, field)| {
                                                    let id = force_id(field.ident.as_ref(), i);
                                                    FieldValue {
                                                        attrs: vec![],
                                                        member: Member::Named(id.clone()),
                                                        colon_token: Some(<Token![:]>::default()),
                                                        expr: expr_of_id(id),
                                                    }
                                                })
                                                .collect(),
                                            dot2_token: None,
                                            rest: None,
                                        }),
                                        Fields::Unnamed(ref fields) => Expr::Call(ExprCall {
                                            attrs: vec![],
                                            func: Box::new(Expr::Path(ExprPath {
                                                attrs: vec![],
                                                qself: None,
                                                path,
                                            })),
                                            paren_token: Paren::default(),
                                            args: fields
                                                .unnamed
                                                .iter()
                                                .enumerate()
                                                .map(|(i, field)| {
                                                    expr_of_id(force_id(field.ident.as_ref(), i))
                                                })
                                                .collect(),
                                        }),
                                    };
                                    Expr::ForLoop(ExprForLoop {
                                        attrs: vec![],
                                        label: None,
                                        for_token: <Token![for]>::default(),
                                        pat: Box::new(tuple_pat.clone()),
                                        in_token: <Token![in]>::default(),
                                        expr: Box::new(Expr::Call(ExprCall {
                                            attrs: vec![],
                                            func: Box::new(Expr::Path(ExprPath {
                                                attrs: vec![],
                                                qself: Some(QSelf {
                                                    lt_token: <Token![<]>::default(),
                                                    ty: Box::new(tuple_type),
                                                    position: 3,
                                                    as_token: Some(<Token![as]>::default()),
                                                    gt_token: <Token![>]>::default(),
                                                }),
                                                path: Path {
                                                    leading_colon: Some(<Token![::]>::default()),
                                                    segments: [
                                                        seg(id("pbt")),
                                                        seg(id("conjure")),
                                                        seg(id("Conjure")),
                                                        seg(id("corners")),
                                                    ]
                                                    .into_iter()
                                                    .collect(),
                                                },
                                            })),
                                            paren_token: Paren::default(),
                                            args: Punctuated::new(),
                                        })),
                                        body: Block {
                                            brace_token: Brace::default(),
                                            stmts: vec![Stmt::Expr(
                                                Expr::If(ExprIf {
                                                    attrs: vec![],
                                                    if_token: <Token![if]>::default(),
                                                    cond: Box::new(Expr::Call(ExprCall {
                                                        attrs: vec![],
                                                        func: Box::new(expr_of_str("property")),
                                                        paren_token: Paren::default(),
                                                        args: iter::once(Expr::Reference(
                                                            ExprReference {
                                                                attrs: vec![],
                                                                and_token: <Token![&]>::default(),
                                                                mutability: None,
                                                                expr: Box::new(instantiate.clone()),
                                                            },
                                                        ))
                                                        .collect(),
                                                    })),
                                                    then_branch: Block {
                                                        brace_token: Brace::default(),
                                                        stmts: vec![Stmt::Expr(
                                                            Expr::Return(ExprReturn {
                                                                attrs: vec![],
                                                                return_token:
                                                                    <Token![return]>::default(),
                                                                expr: Some(Box::new(Expr::Call(
                                                                    ExprCall {
                                                                        attrs: vec![],
                                                                        func: Box::new(
                                                                            expr_of_str("Some"),
                                                                        ),
                                                                        paren_token: Paren::default(
                                                                        ),
                                                                        args: iter::once(
                                                                            instantiate,
                                                                        )
                                                                        .collect(),
                                                                    },
                                                                ))),
                                                            }),
                                                            Some(<Token![;]>::default()),
                                                        )],
                                                    },
                                                    else_branch: None,
                                                }),
                                                None,
                                            )],
                                        },
                                    })
                                })
                                .chain(iter::once(step_fields(path, &variant.fields)))
                                .map(|expr| Stmt::Expr(expr, None))
                                .collect(),
                        },
                    })),
                    comma: Some(<Token![,]>::default()),
                }
            })
            .collect(),
    });
    Block {
        brace_token: Brace::default(),
        stmts: vec![Stmt::Expr(expr, None)],
    }
}

#[inline]
fn step_body_for_struct(item: &ItemStruct) -> Block {
    let (named, punctuated) = match item.fields {
        Fields::Unit => {
            return Block {
                brace_token: Brace::default(),
                stmts: vec![Stmt::Expr(expr_of_str("None"), None)],
            };
        }
        Fields::Named(ref fields) => (true, &fields.named),
        Fields::Unnamed(ref fields) => (false, &fields.unnamed),
    };
    let destructure_self = Local {
        attrs: vec![],
        let_token: <Token![let]>::default(),
        pat: if named {
            Pat::Struct(PatStruct {
                attrs: vec![],
                qself: None,
                path: path_of_str("Self"),
                brace_token: Brace::default(),
                fields: punctuated
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        let id = force_id(field.ident.as_ref(), i);
                        FieldPat {
                            attrs: vec![],
                            member: Member::Named(id.clone()),
                            colon_token: Some(<Token![:]>::default()),
                            pat: Box::new(Pat::Ident(PatIdent {
                                attrs: vec![],
                                by_ref: Some(<Token![ref]>::default()),
                                mutability: None,
                                ident: id,
                                subpat: None,
                            })),
                        }
                    })
                    .collect(),
                rest: None,
            })
        } else {
            Pat::TupleStruct(PatTupleStruct {
                attrs: vec![],
                qself: None,
                path: path_of_str("Self"),
                paren_token: Paren::default(),
                elems: punctuated
                    .iter()
                    .enumerate()
                    .map(|(i, field)| {
                        Pat::Ident(PatIdent {
                            attrs: vec![],
                            by_ref: Some(<Token![ref]>::default()),
                            mutability: None,
                            ident: force_id(field.ident.as_ref(), i),
                            subpat: None,
                        })
                    })
                    .collect(),
            })
        },
        init: Some(LocalInit {
            eq_token: <Token![=]>::default(),
            expr: Box::new(Expr::Unary(ExprUnary {
                attrs: vec![],
                op: UnOp::Deref(<Token![*]>::default()),
                expr: Box::new(expr_of_str("self")),
            })),
            diverge: None,
        }),
        semi_token: <Token![;]>::default(),
    };

    Block {
        brace_token: Brace::default(),
        stmts: vec![
            Stmt::Local(destructure_self),
            Stmt::Expr(
                step_fields(path_of_id(item.ident.clone()), &item.fields),
                None,
            ),
        ],
    }
}

#[inline]
fn cardinality_of_variant(variant: &Variant) -> Expr {
    let tuple = TypeTuple {
        paren_token: Paren::default(),
        elems: variant
            .fields
            .iter()
            .map(|field| ReplaceSelfWithInductive.fold_type(field.ty.clone()))
            .collect(),
    };
    Expr::Path(ExprPath {
        attrs: vec![],
        qself: Some(QSelf {
            lt_token: <Token![<]>::default(),
            ty: Box::new(Type::Tuple(tuple)),
            position: 3,
            as_token: Some(<Token![as]>::default()),
            gt_token: <Token![>]>::default(),
        }),
        path: Path {
            leading_colon: Some(<Token![::]>::default()),
            segments: [
                seg(id("pbt")),
                seg(id("count")),
                seg(id("Count")),
                seg(id("CARDINALITY")),
            ]
            .into_iter()
            .collect(),
        },
    })
}

#[inline]
fn add_extra_bound(mut generics: Generics, extra_bound: &TypeParamBound) -> Generics {
    for param in &mut generics.params {
        if let GenericParam::Type(ref mut ty) = *param {
            let () = ty.bounds.push(extra_bound.clone());
        }
    }
    generics
}

#[inline]
fn params_to_args(
    parameters: &Punctuated<GenericParam, Token![,]>,
) -> Punctuated<GenericArgument, Token![,]> {
    parameters
        .iter()
        .map(|param| match *param {
            GenericParam::Const(ConstParam { ref ident, .. }) => {
                GenericArgument::Const(expr_of_id(ident.clone()))
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
        })
        .collect()
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
fn expr_of_id(ident: Ident) -> Expr {
    Expr::Path(ExprPath {
        attrs: vec![],
        qself: None,
        path: path_of_id(ident),
    })
}

#[inline]
fn expr_of_str(str: &str) -> Expr {
    expr_of_id(id(str))
}
