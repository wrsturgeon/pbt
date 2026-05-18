//! Derive macros for `pbt`.

use {
    core::{iter, num::NonZero},
    heck::{ToSnakeCase as _, ToUpperCamelCase as _},
    proc_macro2::{Span, TokenStream},
    quote::{ToTokens as _, quote},
    syn::{
        AngleBracketedGenericArguments, Arm, Block, ConstParam, Data, DataEnum, DataStruct, Expr,
        ExprArray, ExprBlock, ExprCall, ExprClosure, ExprLit, ExprMatch, ExprMethodCall, ExprPath,
        ExprStruct, Field, FieldPat, FieldValue, Fields, GenericArgument, GenericParam, Generics,
        Ident, Item, LifetimeParam, Lit, LitInt, Local, LocalInit, Macro, MacroDelimiter, Member,
        Pat, PatIdent, PatStruct, PatTuple, PatTupleStruct, Path, PathArguments, PathSegment,
        ReturnType, Stmt, Token, TraitBound, TraitBoundModifier, Type, TypeParam, TypeParamBound,
        TypePath, Visibility, parse_macro_input,
        punctuated::Punctuated,
        spanned::Spanned as _,
        token::{Brace, Bracket, Paren, PathSep},
    },
};

/// The field layout style of one constructor.
#[derive(Clone, Copy)]
enum FieldStyle {
    /// Named fields, e.g. `Variant { field: T }`.
    Named,
    /// No fields, e.g. `Variant`.
    Unit,
    /// Tuple fields, e.g. `Variant(T)`.
    Unnamed,
}

/// One field in a generated shape layer.
struct ShapeField {
    /// Local binding used while pattern matching.
    binding: Ident,
    /// Original source field name, if any.
    ident: Option<Ident>,
    /// Associated type slot used for this field.
    slot_ident: Ident,
}

/// All fields in one generated shape layer.
struct ShapeFields {
    /// Field descriptors in source order.
    fields: Vec<ShapeField>,
    /// Source field style.
    style: FieldStyle,
}

/// One associated type slot in a generated `*Shaped` trait.
struct ShapeSlot {
    /// Concrete field type used by the generated impl.
    concrete_ty: Type,
    /// Associated type identifier.
    ident: Ident,
    /// Bound attached to the associated type declaration, if any.
    trait_bound: Option<TokenStream>,
}

/// An analyzed field type and its associated slot.
struct ShapeType {
    /// Associated type slot for this field type.
    slot_ident: Ident,
}

/// One constructor case in a generated shape interface.
struct ShapeVariant {
    /// Callback type parameter used by the eliminator.
    callback_ident: Ident,
    /// Field shape for this constructor.
    fields: ShapeFields,
    /// Constructor method name.
    method_ident: Ident,
    /// Source constructor name.
    source_ident: Ident,
    /// Generated field-wise layer struct name.
    struct_ident: Ident,
}

/// Complete generated shape support for one item.
struct ShapeSupport {
    /// Unique field type slots used by this item.
    slots: Vec<ShapeSlot>,
    /// Whether the source item is a struct rather than an enum.
    source_is_struct: bool,
    /// Constructor cases.
    variants: Vec<ShapeVariant>,
}

/// One source-level constructor used by `#[derive(Pbt)]` shape generation.
struct SourceCtor<'fields> {
    /// Source fields for this constructor.
    fields: &'fields Fields,
    /// Path used to construct or match this constructor.
    path: Path,
    /// Human-readable source constructor name.
    source_ident: Ident,
    /// Whether this constructor is a whole struct rather than an enum variant.
    source_is_struct: bool,
}

impl ShapeFields {
    /// Construct a value or layer with these fields.
    fn construct(&self, path: TokenStream, values: Vec<TokenStream>) -> TokenStream {
        match self.style {
            FieldStyle::Unit => path,
            FieldStyle::Named => {
                let fields = self.fields.iter().zip(values).map(|(field, value)| {
                    let ident = field.ident.as_ref();
                    quote!(#ident: #value)
                });
                quote!(#path { #(#fields),* })
            }
            FieldStyle::Unnamed => quote!(#path( #(#values),* )),
        }
    }

    /// Pattern match a generated constructor layer.
    fn constructor_pattern(&self, path: TokenStream) -> TokenStream {
        match self.style {
            FieldStyle::Unit => path,
            FieldStyle::Named => {
                let fields = self.fields.iter().filter_map(|field| field.ident.as_ref());
                quote!(#path { #(#fields),* })
            }
            FieldStyle::Unnamed => {
                let bindings = self.fields.iter().map(|field| &field.binding);
                quote!(#path( #(#bindings),* ))
            }
        }
    }

    /// Field values from a matched constructor layer.
    fn constructor_values(&self) -> Vec<TokenStream> {
        self.fields
            .iter()
            .map(|field| {
                field.ident.as_ref().map_or_else(
                    || {
                        let binding = &field.binding;
                        quote!(#binding)
                    },
                    |ident| quote!(#ident),
                )
            })
            .collect()
    }

    /// Pattern match a borrowed source value.
    fn source_ref_pattern(&self, path: TokenStream) -> TokenStream {
        match self.style {
            FieldStyle::Unit => path,
            FieldStyle::Named => {
                let fields = self.fields.iter().map(|field| {
                    let ident = field.ident.as_ref();
                    let binding = &field.binding;
                    quote!(#ident: ref #binding)
                });
                quote!(#path { #(#fields),* })
            }
            FieldStyle::Unnamed => {
                let bindings = self.fields.iter().map(|field| {
                    let binding = &field.binding;
                    quote!(ref #binding)
                });
                quote!(#path( #(#bindings),* ))
            }
        }
    }

    /// Generated fields for this layer struct.
    fn struct_fields(&self, vis: &Visibility) -> TokenStream {
        match self.style {
            FieldStyle::Unit => quote!(),
            FieldStyle::Named => {
                let fields = self.fields.iter().map(|field| {
                    let ident = field.ident.as_ref();
                    let slot = &field.slot_ident;
                    let doc = shape_field_doc(field);
                    quote!(#[doc = #doc] #vis #ident: #slot)
                });
                quote!({ #(#fields),* })
            }
            FieldStyle::Unnamed => {
                let fields = self.fields.iter().map(|field| {
                    let slot = &field.slot_ident;
                    let doc = shape_field_doc(field);
                    quote!(#[doc = #doc] #vis #slot)
                });
                quote!(( #(#fields),* );)
            }
        }
    }

    /// Unique associated type slots used by this layer.
    fn unique_slots(&self) -> Vec<&Ident> {
        let mut seen = Vec::new();
        let mut out = Vec::new();
        for field in &self.fields {
            let slot = field.slot_ident.to_string();
            if !seen.contains(&slot) {
                seen.push(slot);
                out.push(&field.slot_ident);
            }
        }
        out
    }
}

impl ShapeVariant {
    /// The generated layer type applied to `args`.
    fn applied_type(&self, args: &[TokenStream]) -> TokenStream {
        let struct_ident = &self.struct_ident;
        if args.is_empty() {
            quote!(#struct_ident)
        } else {
            quote!(#struct_ident<#(#args),*>)
        }
    }

    /// The generated layer type with associated type fields.
    fn associated_type(&self) -> TokenStream {
        let args = self
            .fields
            .unique_slots()
            .into_iter()
            .map(|slot| quote!(Self::#slot))
            .collect::<Vec<_>>();
        self.applied_type(&args)
    }

    /// The generated layer type with borrowed associated type fields.
    fn borrowed_type(&self) -> TokenStream {
        let args = self
            .fields
            .unique_slots()
            .into_iter()
            .map(|slot| quote!(&Self::#slot))
            .collect::<Vec<_>>();
        self.applied_type(&args)
    }

    /// Constructor method declaration.
    fn constructor_decl(&self) -> TokenStream {
        let method_ident = &self.method_ident;
        let struct_ty = self.associated_type();
        let source_name = self.source_ident.to_string();
        let doc = format!("Construct the `{source_name}` case from its field-wise shape.");
        quote! {
            #[doc = #doc]
            fn #method_ident(layer: #struct_ty) -> Self;
        }
    }

    /// Constructor method implementation.
    fn constructor_impl(&self, original_ident: &Ident, source_is_struct: bool) -> TokenStream {
        let method_ident = &self.method_ident;
        let struct_ty = self.associated_type();
        let source_path = shape_source_path(original_ident, &self.source_ident, source_is_struct);
        let constructor_values = self.fields.constructor_values();
        let constructed = self.fields.construct(source_path, constructor_values);

        if self.fields.fields.is_empty() {
            quote! {
                #[inline(always)]
                fn #method_ident(_: #struct_ty) -> Self {
                    #constructed
                }
            }
        } else {
            let struct_ident = &self.struct_ident;
            let pattern = self.fields.constructor_pattern(quote!(#struct_ident));
            quote! {
                #[inline(always)]
                fn #method_ident(#pattern: #struct_ty) -> Self {
                    #constructed
                }
            }
        }
    }

    /// Eliminator callback parameter.
    fn elim_callback_param(&self) -> TokenStream {
        let callback_ident = &self.callback_ident;
        let method_ident = &self.method_ident;
        quote!(#method_ident: #callback_ident)
    }

    /// Eliminator callback bound.
    fn elim_callback_predicate(&self) -> TokenStream {
        let callback_ident = &self.callback_ident;
        let borrowed_ty = self.borrowed_type();
        quote!(#callback_ident: FnOnce(State, #borrowed_ty) -> Output)
    }

    /// Match arm for an enum eliminator.
    fn elim_enum_arm(&self, original_ident: &Ident) -> TokenStream {
        let method_ident = &self.method_ident;
        let source_ident = &self.source_ident;
        let source_path = quote!(#original_ident::#source_ident);
        let pattern = self.fields.source_ref_pattern(source_path);
        let values = self.fields.fields.iter().map(|field| {
            let binding = &field.binding;
            quote!(#binding)
        });
        let struct_ident = &self.struct_ident;
        let layer = self
            .fields
            .construct(quote!(#struct_ident), values.collect());

        quote!(#pattern => #method_ident(state, #layer))
    }

    /// Body for a struct eliminator.
    fn elim_struct_body(&self, original_ident: &Ident) -> TokenStream {
        let method_ident = &self.method_ident;
        let pattern = self.fields.source_ref_pattern(quote!(#original_ident));
        let values = self.fields.fields.iter().map(|field| {
            let binding = &field.binding;
            quote!(#binding)
        });
        let struct_ident = &self.struct_ident;
        let layer = self
            .fields
            .construct(quote!(#struct_ident), values.collect());

        quote! {
            let #pattern = *self;
            #method_ident(state, #layer)
        }
    }

    /// Generated shape layer struct definition.
    fn struct_definition(&self, vis: &Visibility) -> TokenStream {
        let struct_ident = &self.struct_ident;
        let slots = self.fields.unique_slots();
        let fields = self.fields.struct_fields(vis);
        let source_name = self.source_ident.to_string();
        let doc = format!("Field-wise shape for the `{source_name}` case.");

        if matches!(self.fields.style, FieldStyle::Unit) {
            return quote! {
                #[doc = #doc]
                #[allow(
                    clippy::exhaustive_structs,
                    reason = "generated shape structs are public field-wise constructors"
                )]
                #vis struct #struct_ident;
            };
        }

        if slots.is_empty() {
            return quote! {
                #[doc = #doc]
                #[allow(
                    clippy::exhaustive_structs,
                    reason = "generated shape structs are public field-wise constructors"
                )]
                #vis struct #struct_ident #fields
            };
        }

        quote! {
            #[doc = #doc]
            #[allow(
                clippy::exhaustive_structs,
                reason = "generated shape structs are public field-wise constructors"
            )]
            #vis struct #struct_ident<#(#slots),*> #fields
        }
    }
}

/// Extract source constructors from one source item.
fn source_ctors_for_data<'fields>(
    ident: &Ident,
    data: &'fields Data,
) -> syn::Result<Vec<SourceCtor<'fields>>> {
    match *data {
        Data::Enum(ref enum_data) => Ok(enum_data
            .variants
            .iter()
            .map(|variant| SourceCtor {
                fields: &variant.fields,
                path: Path {
                    leading_colon: None,
                    segments: [seg(id("Self")), seg(variant.ident.clone())]
                        .into_iter()
                        .collect(),
                },
                source_ident: variant.ident.clone(),
                source_is_struct: false,
            })
            .collect()),
        Data::Struct(ref struct_data) => Ok(vec![SourceCtor {
            fields: &struct_data.fields,
            path: path_of_str("Self"),
            source_ident: ident.clone(),
            source_is_struct: true,
        }]),
        Data::Union(ref union_data) => Err(syn::Error::new_spanned(
            union_data.union_token,
            "`pbt` macros do not support unions",
        )),
    }
}

/// Analyze all fields in a shaped item.
fn analyze_shape(ident: &Ident, generics: &Generics, data: &Data) -> syn::Result<ShapeSupport> {
    let generic_names = type_param_idents(generics);
    let source_ctors = source_ctors_for_data(ident, data)?;
    let source_is_struct = source_ctors
        .first()
        .is_some_and(|ctor| ctor.source_is_struct);
    let mut slots = Vec::new();
    let variants = source_ctors
        .iter()
        .map(|ctor| {
            let fields = analyze_shape_fields(ident, ctor.fields, &generic_names, &mut slots)?;
            Ok(ShapeVariant {
                callback_ident: shape_callback_ident(&ctor.source_ident),
                fields,
                method_ident: shape_method_ident(&ctor.source_ident),
                source_ident: ctor.source_ident.clone(),
                struct_ident: if ctor.source_is_struct {
                    format_ident_from_str(&format!("{ident}Shape"))
                } else {
                    shape_enum_variant_struct_ident(ident, &ctor.source_ident)
                },
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(ShapeSupport {
        slots,
        source_is_struct,
        variants,
    })
}

/// Analyze one source field for shape generation.
fn analyze_shape_field(
    original_ident: &Ident,
    field: &Field,
    index: usize,
    generic_names: &[String],
    slots: &mut Vec<ShapeSlot>,
) -> syn::Result<ShapeField> {
    let slot_ident = shape_slot_for_type(original_ident, &field.ty, generic_names, slots)?;

    Ok(ShapeField {
        binding: id(&format!("__pbt_shaped_field_{index}")),
        ident: field.ident.clone(),
        slot_ident,
    })
}

/// Analyze all source fields for one shaped constructor.
fn analyze_shape_fields(
    original_ident: &Ident,
    source_fields: &Fields,
    generic_names: &[String],
    slots: &mut Vec<ShapeSlot>,
) -> syn::Result<ShapeFields> {
    let (style, analyzed_fields) = match *source_fields {
        Fields::Named(ref named_fields) => {
            let analyzed = named_fields
                .named
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    analyze_shape_field(original_ident, field, index, generic_names, slots)
                })
                .collect::<syn::Result<Vec<_>>>()?;
            (FieldStyle::Named, analyzed)
        }
        Fields::Unnamed(ref unnamed_fields) => {
            let analyzed = unnamed_fields
                .unnamed
                .iter()
                .enumerate()
                .map(|(index, field)| {
                    analyze_shape_field(original_ident, field, index, generic_names, slots)
                })
                .collect::<syn::Result<Vec<_>>>()?;
            (FieldStyle::Unnamed, analyzed)
        }
        Fields::Unit => (FieldStyle::Unit, Vec::new()),
    };

    Ok(ShapeFields {
        fields: analyzed_fields,
        style,
    })
}

/// Declare one shape associated type.
fn associated_shape_type(slot: &ShapeSlot) -> TokenStream {
    let ident = &slot.ident;
    let doc = format!("Field type slot `{ident}` for this shape.");
    slot.trait_bound.as_ref().map_or_else(
        || {
            quote! {
                #[doc = #doc]
                type #ident;
            }
        },
        |trait_bound| {
            quote! {
                #[doc = #doc]
                type #ident: #trait_bound;
            }
        },
    )
}

/// Implement one shape associated type.
fn associated_shape_type_impl(slot: &ShapeSlot) -> TokenStream {
    let ident = &slot.ident;
    let ty = &slot.concrete_ty;
    quote!(type #ident = #ty;)
}

/// Field-type `Pbt` bounds required by generated shape defaults.
fn shape_pbt_bounds(support: &ShapeSupport) -> Vec<TokenStream> {
    support
        .slots
        .iter()
        .map(|slot| {
            let ident = &slot.ident;
            quote!(Self::#ident: ::pbt::pbt::Pbt)
        })
        .collect()
}

/// Generate direct arbitrary construction for one shaped constructor.
fn shape_arbitrary_ctor(variant: &ShapeVariant) -> TokenStream {
    let method_ident = &variant.method_ident;
    let struct_ident = &variant.struct_ident;
    let values = variant.fields.fields.iter().map(|field| {
        let slot = &field.slot_ident;
        quote!(::pbt::pbt::arbitrary_field::<Self::#slot>(&mut sizes, prng)?)
    });
    let layer = variant
        .fields
        .construct(quote!(#struct_ident), values.collect());
    quote!(Self::#method_ident(#layer))
}

/// Generate erased-field construction for one shaped constructor.
fn shape_call_ctor(variant: &ShapeVariant) -> TokenStream {
    let method_ident = &variant.method_ident;
    let struct_ident = &variant.struct_ident;
    let values = variant.fields.fields.iter().map(|field| {
        let slot = &field.slot_ident;
        quote!(terms.must_pop::<Self::#slot>())
    });
    let layer = variant
        .fields
        .construct(quote!(#struct_ident), values.collect());
    quote!(Self::#method_ident(#layer))
}

/// Build the constructor metadata emitted by a shaped default type former.
fn shape_introduction_rules(support: &ShapeSupport) -> TokenStream {
    let rules = support.variants.iter().map(|variant| {
        let arbitrary_ctor = shape_arbitrary_ctor(variant);
        let call_ctor = shape_call_ctor(variant);
        let dependencies = variant.fields.fields.iter().map(|field| {
            let slot = &field.slot_ident;
            quote!(::pbt::reflection::type_of::<Self::#slot>())
        });
        quote! {
            ::pbt::pbt::IntroductionRule {
                arbitrary: ::pbt::pbt::ArbitraryFn::new(|prng, mut sizes| {
                    Ok(Some(#arbitrary_ctor))
                }),
                call: ::pbt::pbt::CtorFn::new(|terms| Some(#call_ctor)),
                immediate_dependencies: [#(#dependencies),*].into_iter().collect(),
            }
        }
    });
    quote!(vec![#(#rules),*])
}

/// Pattern match a borrowed shaped layer and push owned clones into erased buckets.
fn shape_elim_callback_body(
    variant: &ShapeVariant,
    nonzero_ctor_index: NonZero<usize>,
) -> TokenStream {
    let ctor_index = nonzero_ctor_index.get();
    let struct_ident = &variant.struct_ident;
    let pattern = variant.fields.constructor_pattern(quote!(#struct_ident));
    let pushes = variant.fields.fields.iter().rev().map(|field| {
        let binding = field.ident.as_ref().unwrap_or(&field.binding);
        let slot = &field.slot_ident;
        quote!(let () = fields.push::<Self::#slot>((*#binding).clone());)
    });
    quote! {
        |(), #pattern| {
            let mut fields = ::pbt::reflection::TermsOfVariousTypes::new();
            #(#pushes)*
            (#ctor_index, fields)
        }
    }
}

/// Build the eliminator emitted by a shaped default type former.
fn shape_elimination_rule(support: &ShapeSupport) -> TokenStream {
    if support.variants.is_empty() {
        return quote!(::pbt::pbt::ElimFn::new(|uninhabited| match uninhabited {}));
    }

    let callbacks = support
        .variants
        .iter()
        .enumerate()
        .map(|(zero_based_index, variant)| {
            // SAFETY: Adding 1.
            let ctor_index = unsafe {
                NonZero::new_unchecked(
                    #[expect(clippy::expect_used, reason = "extremely unlikely")]
                    zero_based_index
                        .checked_add(1)
                        .expect("internal `pbt` error: more than `usize::MAX` constructors"),
                )
            };
            shape_elim_callback_body(variant, ctor_index)
        });
    quote! {
        ::pbt::pbt::ElimFn::new(|constructed| {
            let (ctor_idx, fields): (usize, ::pbt::reflection::TermsOfVariousTypes) =
                constructed.elim((), #(#callbacks),*);
            ::pbt::pbt::Decomposition {
                // SAFETY: Case analysis above.
                ctor_idx: unsafe { ::core::num::NonZero::new_unchecked(ctor_idx) },
                fields,
            }
        })
    }
}

/// Build the default shaped `type_former` body.
fn shape_type_former_body(support: &ShapeSupport) -> TokenStream {
    let introduction_rules = shape_introduction_rules(support);
    let elimination_rule = shape_elimination_rule(support);
    quote! {
        ::pbt::pbt::TypeFormer::Algebraic(::pbt::pbt::Algebraic {
            introduction_rules: #introduction_rules,
            elimination_rule: #elimination_rule,
        })
    }
}

/// Build the default shaped immediate-dependency registration body.
fn shape_register_body(support: &ShapeSupport) -> TokenStream {
    let registers = support
        .variants
        .iter()
        .flat_map(|variant| variant.fields.fields.iter())
        .map(|field| {
            let slot = &field.slot_ident;
            quote!(let () = ::pbt::reflection::register::<Self::#slot>(visited.clone(), sccs);)
        });
    quote! {
        if !visited.insert(::pbt::reflection::type_of::<Self>()) {
            return;
        }
        #(#registers)*
    }
}

/// Build one shaped `visit_deep` callback.
fn shape_visit_callback(variant: &ShapeVariant) -> TokenStream {
    let struct_ident = &variant.struct_ident;
    let pattern = variant.fields.constructor_pattern(quote!(#struct_ident));
    let iter = variant
        .fields
        .fields
        .iter()
        .fold(quote!(::core::iter::empty()), |acc, field| {
            let binding = field.ident.as_ref().unwrap_or(&field.binding);
            quote!(::pbt::pbt::Pbt::visit_deep(#binding).chain(#acc))
        });
    quote! {
        |(), #pattern| {
            #iter.collect::<Vec<_>>()
        }
    }
}

/// Build the default shaped `visit_deep` body.
fn shape_visit_body(support: &ShapeSupport) -> TokenStream {
    if support.variants.is_empty() {
        return quote!(
            ::pbt::pbt::visit_self_opt::<V, Self>(self)
                .cloned()
                .into_iter()
                .chain(::core::iter::empty())
        );
    }

    let callbacks = support.variants.iter().map(shape_visit_callback);
    quote! {
        ::pbt::pbt::visit_self_opt::<V, Self>(self).cloned().into_iter().chain({
            self.elim((), #(#callbacks),*).into_iter()
        })
    }
}

/// Generate hidden default PBT-like methods for a shaped trait.
fn shape_pbt_like_methods(support: &ShapeSupport) -> TokenStream {
    let bounds = shape_pbt_bounds(support);
    let register_body = shape_register_body(support);
    let type_former_body = shape_type_former_body(support);
    let visit_body = shape_visit_body(support);
    quote! {
        #[doc(hidden)]
        fn shape_register_all_immediate_dependencies(
            visited: &mut ::std::collections::BTreeSet<::pbt::reflection::Type>,
            sccs: &mut ::pbt::StronglyConnectedComponents,
        )
        where
            Self: 'static + ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::Eq + Sized,
            #(#bounds),*
        {
            #register_body
        }

        #[doc(hidden)]
        fn shape_type_former() -> ::pbt::pbt::TypeFormer<Self>
        where
            Self: 'static + ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::Eq + Sized,
            #(#bounds),*
        {
            #type_former_body
        }

        #[doc(hidden)]
        fn shape_visit_deep<V>(&self) -> impl ::core::iter::Iterator<Item = V> + '_
        where
            Self: 'static + ::core::clone::Clone + ::core::fmt::Debug + ::core::cmp::Eq + Sized,
            V: ::pbt::pbt::Pbt,
            #(#bounds),*
        {
            #visit_body
        }
    }
}

/// Generate shape support for one item.
#[expect(
    clippy::cognitive_complexity,
    reason = "shape support assembles one cohesive generated trait and impl"
)]
fn shaped_support(
    vis: &Visibility,
    ident: &Ident,
    generics: &Generics,
    data: &Data,
) -> syn::Result<TokenStream> {
    let support = analyze_shape(ident, generics, data)?;
    let trait_ident = shape_trait_ident(ident);
    let trait_doc = format!("Field-wise shape interface for `{ident}` values.");
    let elim_doc =
        format!("Eliminate a `{ident}` by dispatching to the callback for its current case.");
    let struct_defs = support
        .variants
        .iter()
        .map(|variant| variant.struct_definition(vis));
    let assoc_decls = support.slots.iter().map(associated_shape_type);
    let constructor_decls = support.variants.iter().map(ShapeVariant::constructor_decl);
    let callback_idents = support
        .variants
        .iter()
        .map(|variant| &variant.callback_ident)
        .collect::<Vec<_>>();
    let callback_params = support
        .variants
        .iter()
        .map(ShapeVariant::elim_callback_param)
        .collect::<Vec<_>>();
    let callback_predicates = support
        .variants
        .iter()
        .map(ShapeVariant::elim_callback_predicate)
        .collect::<Vec<_>>();
    let shape_defaults = shape_pbt_like_methods(&support);
    let (impl_generics, type_generics, impl_where) = generics.split_for_impl();
    let assoc_impls = support.slots.iter().map(associated_shape_type_impl);
    let constructor_impls = support
        .variants
        .iter()
        .map(|variant| variant.constructor_impl(ident, support.source_is_struct));
    let self_ty = quote!(#ident #type_generics);
    let elim_body = if support.source_is_struct {
        let Some(variant) = support.variants.first() else {
            return Err(syn::Error::new_spanned(
                ident,
                "shape generation does not support empty structs",
            ));
        };
        variant.elim_struct_body(ident)
    } else if support.variants.is_empty() {
        quote! {
            let _ = state;
            match *self {}
        }
    } else {
        let arms = support
            .variants
            .iter()
            .map(|variant| variant.elim_enum_arm(ident));
        quote! {
            match *self {
                #(#arms),*
            }
        }
    };

    let trait_items = if support.variants.is_empty() {
        quote! {
            #(#assoc_decls)*
            #[doc = #elim_doc]
            fn elim<Output, State>(&self, state: State) -> Output;
        }
    } else {
        quote! {
            #(#assoc_decls)*
            #(#constructor_decls)*

            #[doc = #elim_doc]
            fn elim<Output, State #(, #callback_idents)*>(
                &self,
                state: State #(, #callback_params)*,
            ) -> Output
            where
                #(#callback_predicates),*;

            #shape_defaults
        }
    };

    let impl_items = if support.variants.is_empty() {
        quote! {
            #(#assoc_impls)*

            #[inline(always)]
            fn elim<Output, State>(&self, state: State) -> Output {
                #elim_body
            }
        }
    } else {
        quote! {
            #(#assoc_impls)*
            #(#constructor_impls)*

            #[inline(always)]
            fn elim<Output, State #(, #callback_idents)*>(
                &self,
                state: State #(, #callback_params)*,
            ) -> Output
            where
                #(#callback_predicates),*
            {
                #elim_body
            }
        }
    };

    Ok(quote! {
        #(#struct_defs)*

        #[doc = #trait_doc]
        #vis trait #trait_ident {
            #trait_items
        }

        impl #impl_generics #trait_ident for #self_ty #impl_where {
            #impl_items
        }
    })
}

/// Insert a unique shape slot, returning its associated type identifier.
fn insert_shape_slot(slots: &mut Vec<ShapeSlot>, slot: ShapeSlot) -> syn::Result<Ident> {
    if let Some(existing) = slots.iter().find(|existing| existing.ident == slot.ident) {
        let existing_ty = existing.concrete_ty.to_token_stream().to_string();
        let slot_ty = slot.concrete_ty.to_token_stream().to_string();
        if existing_ty == slot_ty
            && existing.trait_bound.as_ref().map(ToString::to_string)
                == slot.trait_bound.as_ref().map(ToString::to_string)
        {
            return Ok(existing.ident.clone());
        }
        return Err(syn::Error::new_spanned(
            slot.ident,
            "generated shaped type slot name collides with a different field type",
        ));
    }

    let ident = slot.ident.clone();
    slots.push(slot);
    Ok(ident)
}

/// Callback type parameter name for one constructor.
fn shape_callback_ident(ident: &Ident) -> Ident {
    format_ident_from_str(&format!(
        "{}Continuation",
        ident_fragment(ident).to_upper_camel_case()
    ))
}

/// Documentation for one generated layer field.
fn shape_field_doc(field: &ShapeField) -> String {
    field.ident.as_ref().map_or_else(
        || String::from("Positional field in this shape case."),
        |ident| format!("Field `{ident}` in this shape case."),
    )
}

/// Generated layer struct name for one enum variant.
fn shape_enum_variant_struct_ident(original_ident: &Ident, variant_ident: &Ident) -> Ident {
    let original = ident_fragment(original_ident);
    let variant = ident_fragment(variant_ident);
    format_ident_from_str(&format!("{original}{variant}"))
}

/// Constructor method name for one source constructor.
fn shape_method_ident(ident: &Ident) -> Ident {
    format_ident_from_str(&ident_fragment(ident).to_snake_case())
}

/// Source constructor path.
fn shape_source_path(
    original_ident: &Ident,
    variant_ident: &Ident,
    source_is_struct: bool,
) -> TokenStream {
    if source_is_struct {
        quote!(#original_ident)
    } else {
        quote!(#original_ident::#variant_ident)
    }
}

/// Generated shape trait name for one source item or field type.
fn shape_trait_ident(ident: &Ident) -> Ident {
    format_ident_from_str(&format!(
        "{}Shaped",
        ident_fragment(ident).to_upper_camel_case()
    ))
}

/// Shape trait bound for a well-known standard field type.
fn well_known_shape_trait_bound(ident: &Ident) -> Option<TokenStream> {
    match ident.to_string().as_str() {
        "Arc" => Some(quote!(::pbt::shape::ArcShaped)),
        "bool" => Some(quote!(::pbt::shape::BoolShaped)),
        "Box" => Some(quote!(::pbt::shape::BoxShaped)),
        "char" => Some(quote!(::pbt::shape::CharShaped)),
        "CString" => Some(quote!(::pbt::shape::CStringShaped)),
        "i8" => Some(quote!(::pbt::shape::I8Shaped)),
        "i16" => Some(quote!(::pbt::shape::I16Shaped)),
        "i32" => Some(quote!(::pbt::shape::I32Shaped)),
        "i64" => Some(quote!(::pbt::shape::I64Shaped)),
        "i128" => Some(quote!(::pbt::shape::I128Shaped)),
        "Infallible" => Some(quote!(::pbt::shape::InfallibleShaped)),
        "isize" => Some(quote!(::pbt::shape::IsizeShaped)),
        "NonZero" => Some(quote!(::pbt::shape::NonZeroShaped)),
        "Option" => Some(quote!(::pbt::shape::OptionShaped)),
        "PhantomData" => Some(quote!(::pbt::shape::PhantomDataShaped)),
        "Rc" => Some(quote!(::pbt::shape::RcShaped)),
        "u8" => Some(quote!(::pbt::shape::U8Shaped)),
        "u16" => Some(quote!(::pbt::shape::U16Shaped)),
        "u32" => Some(quote!(::pbt::shape::U32Shaped)),
        "u64" => Some(quote!(::pbt::shape::U64Shaped)),
        "u128" => Some(quote!(::pbt::shape::U128Shaped)),
        "usize" => Some(quote!(::pbt::shape::UsizeShaped)),
        "Vec" => Some(quote!(::pbt::shape::VecShaped)),
        _ => None,
    }
}

/// Shape type for a field whose internal shape is not known to `pbt`.
fn opaque_shape_type(ty: &Type, slots: &mut Vec<ShapeSlot>) -> syn::Result<ShapeType> {
    let slot = ShapeSlot {
        concrete_ty: ty.clone(),
        ident: shape_opaque_slot_ident(ty),
        trait_bound: None,
    };
    let slot_ident = insert_shape_slot(slots, slot)?;
    Ok(ShapeType { slot_ident })
}

/// Associated type slot name for an opaque field type.
fn shape_opaque_slot_ident(ty: &Type) -> Ident {
    format_ident_from_str(&type_fragment(ty))
}

/// Associated type slot name for a field type.
fn shape_type_slot_ident(ident: &Ident, arg_slots: &[Ident]) -> Ident {
    let mut fragment = ident_fragment(ident).to_upper_camel_case();
    for arg_slot in arg_slots {
        fragment.push_str(&ident_fragment(arg_slot));
    }
    format_ident_from_str(&fragment)
}

/// Slot identifier for one field type.
fn shape_slot_for_type(
    original_ident: &Ident,
    ty: &Type,
    generic_names: &[String],
    slots: &mut Vec<ShapeSlot>,
) -> syn::Result<Ident> {
    Ok(shape_type_for_type(original_ident, ty, generic_names, slots)?.slot_ident)
}

/// Analyze one field type for shape generation.
fn shape_type_for_type(
    original_ident: &Ident,
    ty: &Type,
    generic_names: &[String],
    slots: &mut Vec<ShapeSlot>,
) -> syn::Result<ShapeType> {
    if is_direct_self(ty) {
        let concrete_ty = Type::Verbatim(quote!(Self));
        let trait_ident = shape_trait_ident(original_ident);
        let slot = ShapeSlot {
            concrete_ty,
            ident: original_ident.clone(),
            trait_bound: Some(quote!(#trait_ident)),
        };
        let slot_ident = insert_shape_slot(slots, slot)?;
        return Ok(ShapeType { slot_ident });
    }

    let Type::Path(ref path_type) = *ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "shape generation supports only direct `Self` and path field types for now",
        ));
    };

    if path_type.qself.is_some() {
        return opaque_shape_type(ty, slots);
    }

    let Some((last, rest)) = path_type.path.segments.iter().next_back().map(|last| {
        let rest = path_type
            .path
            .segments
            .iter()
            .take(path_type.path.segments.len().saturating_sub(1));
        (last, rest)
    }) else {
        return Err(syn::Error::new_spanned(ty, "expected a type path segment"));
    };

    if last.ident == "Self" {
        return Err(syn::Error::new_spanned(
            ty,
            "unsupported qualified use of `Self` in a field type",
        ));
    }

    let is_generic = generic_names.contains(&last.ident.to_string());
    let well_known_trait = well_known_shape_trait_bound(&last.ident);
    if !is_generic && well_known_trait.is_none() {
        return opaque_shape_type(ty, slots);
    }

    for segment in rest {
        if !matches!(segment.arguments, PathArguments::None) {
            return Err(syn::Error::new_spanned(
                ty,
                "shape generation supports generic arguments only on the final field path segment",
            ));
        }
    }

    let arg_shapes = shape_type_arguments(original_ident, &last.arguments, generic_names, slots)?;
    let arg_slots = arg_shapes
        .iter()
        .map(|shape| shape.slot_ident.clone())
        .collect::<Vec<_>>();
    let ident = shape_type_slot_ident(&last.ident, &arg_slots);
    let trait_bound = well_known_trait.map(|trait_base| {
        if arg_slots.is_empty() {
            quote!(#trait_base)
        } else {
            let trait_args = arg_slots.iter().map(|slot| quote!(Self::#slot));
            quote!(#trait_base<#(#trait_args),*>)
        }
    });
    let slot = ShapeSlot {
        concrete_ty: ty.clone(),
        ident,
        trait_bound,
    };
    let slot_ident = insert_shape_slot(slots, slot)?;

    Ok(ShapeType { slot_ident })
}

/// Analyze generic arguments in one field type for shape generation.
fn shape_type_arguments(
    original_ident: &Ident,
    arguments: &PathArguments,
    generic_names: &[String],
    slots: &mut Vec<ShapeSlot>,
) -> syn::Result<Vec<ShapeType>> {
    match *arguments {
        PathArguments::None => Ok(Vec::new()),
        PathArguments::AngleBracketed(ref angle_args) => angle_args
            .args
            .iter()
            .map(|arg| match *arg {
                GenericArgument::Type(ref arg_ty) => {
                    shape_type_for_type(original_ident, arg_ty, generic_names, slots)
                }
                _ => Err(syn::Error::new_spanned(
                    arg,
                    "shape generation supports only type generic arguments in field paths for now",
                )),
            })
            .collect(),
        PathArguments::Parenthesized(ref parenthesized_args) => Err(syn::Error::new_spanned(
            parenthesized_args,
            "shape generation does not support parenthesized field path arguments yet",
        )),
    }
}

/// Whether a type is exactly `Self`.
fn is_direct_self(ty: &Type) -> bool {
    let Type::Path(ref path_type) = *ty else {
        return false;
    };
    if path_type.qself.is_some() || path_type.path.segments.len() != 1 {
        return false;
    }
    let Some(segment) = path_type.path.segments.first() else {
        return false;
    };
    segment.ident == "Self" && matches!(segment.arguments, PathArguments::None)
}

/// Stable string fragment for an identifier.
fn ident_fragment(ident: &Ident) -> String {
    ident.to_string().trim_start_matches("r#").to_owned()
}

/// Stable string fragment for an arbitrary type.
fn type_fragment(ty: &Type) -> String {
    token_fragment(&ty.to_token_stream())
}

/// Stable string fragment for arbitrary Rust tokens.
fn token_fragment(tokens: &TokenStream) -> String {
    let mut fragment = String::new();
    let mut upper_next = true;

    for character in tokens.to_string().chars() {
        if !character.is_ascii_alphanumeric() {
            upper_next = true;
            continue;
        }
        if fragment.is_empty() && character.is_ascii_digit() {
            fragment.push_str("Type");
        }
        if upper_next {
            fragment.push(character.to_ascii_uppercase());
            upper_next = false;
        } else {
            fragment.push(character);
        }
    }

    if fragment.is_empty() {
        String::from("Type")
    } else {
        fragment
    }
}

/// Create an identifier from a string.
fn format_ident_from_str(s: &str) -> Ident {
    Ident::new(s, Span::call_site())
}

/// Derive all necessary traits in the `pbt` crate.
/// # Panics
/// If the annotated item is neither an `enum` nor a `struct`.
#[proc_macro_derive(Pbt)]
#[inline]
pub fn derive_pbt(ts: proc_macro::TokenStream) -> proc_macro::TokenStream {
    match expand_pbt(parse_macro_input!(ts as Item)) {
        Ok(expanded) => expanded,
        Err(error) => error.into_compile_error(),
    }
    .into()
}

/// Expand one `#[derive(Pbt)]` item.
fn expand_pbt(source_item: Item) -> syn::Result<TokenStream> {
    match source_item {
        Item::Enum(enum_item) => {
            let vis = enum_item.vis;
            let ident = enum_item.ident;
            let generics = enum_item.generics;
            let data = Data::Enum(DataEnum {
                enum_token: enum_item.enum_token,
                brace_token: enum_item.brace_token,
                variants: enum_item.variants,
            });
            derive_pbt_for_data(&vis, &ident, &generics, &data)
        }
        Item::Struct(struct_item) => {
            let vis = struct_item.vis;
            let ident = struct_item.ident;
            let generics = struct_item.generics;
            let data = Data::Struct(DataStruct {
                struct_token: struct_item.struct_token,
                fields: struct_item.fields,
                semi_token: struct_item.semi_token,
            });
            derive_pbt_for_data(&vis, &ident, &generics, &data)
        }
        other_item => Err(syn::Error::new(
            other_item.span(),
            "`#[derive(Pbt)]` expects an `enum` or a `struct`",
        )),
    }
}

/// Generate the `Pbt` implementation for the given data.
#[inline]
fn derive_pbt_for_data(
    vis: &Visibility,
    ident: &Ident,
    generics: &Generics,
    data: &Data,
) -> syn::Result<TokenStream> {
    let shaped = shaped_support(vis, ident, generics, data)?;
    let source_ctors = source_ctors_for_data(ident, data)?;
    let pbt_impl = if source_ctors.is_empty() {
        derive_pbt_for_ctors(ident, generics, &source_ctors)
    } else {
        derive_pbt_impl_from_shape(ident, generics)
    };
    Ok(quote! {
        #shaped
        #pbt_impl
    })
}

/// Generate the `Pbt` implementation by delegating to generated shape defaults.
#[inline]
fn derive_pbt_impl_from_shape(ident: &Ident, generics: &Generics) -> TokenStream {
    let construct_trait_path = Path {
        leading_colon: Some(PathSep::default()),
        segments: [seg(id("pbt")), seg(id("pbt")), seg(id("Pbt"))]
            .into_iter()
            .collect(),
    };
    let parameters = generics_to_parameters(generics);
    let bounded_generics = add_construct_bound_to_each_generic(generics, &construct_trait_path);
    let generated_tests = generated_tests(ident, &bounded_generics);
    let trait_ident = shape_trait_ident(ident);
    let impl_path = Path {
        leading_colon: None,
        segments: iter::once(PathSegment {
            ident: ident.clone(),
            arguments: PathArguments::AngleBracketed(parameters),
        })
        .collect(),
    };

    quote! {
        impl #bounded_generics #construct_trait_path for #impl_path {
            #[inline]
            fn register_all_immediate_dependencies(
                visited: &mut ::std::collections::BTreeSet<::pbt::reflection::Type>,
                sccs: &mut ::pbt::StronglyConnectedComponents,
            ) {
                <Self as #trait_ident>::shape_register_all_immediate_dependencies(visited, sccs)
            }

            #[inline]
            fn type_former() -> ::pbt::pbt::TypeFormer<Self> {
                <Self as #trait_ident>::shape_type_former()
            }

            #[inline]
            fn visit_deep<V>(&self) -> impl ::core::iter::Iterator<Item = V>
            where
                V: ::pbt::pbt::Pbt,
            {
                <Self as #trait_ident>::shape_visit_deep(self)
            }
        }

        #generated_tests
    }
}

/// Generate the `Pbt` implementation for the given constructors.
#[inline]
fn derive_pbt_for_ctors(
    ident: &Ident,
    generics: &Generics,
    ctors: &[SourceCtor<'_>],
) -> TokenStream {
    let construct_trait_path = Path {
        leading_colon: Some(PathSep::default()),
        segments: [seg(id("pbt")), seg(id("pbt")), seg(id("Pbt"))]
            .into_iter()
            .collect(),
    };
    let parameters = generics_to_parameters(generics);
    let bounded_generics = add_construct_bound_to_each_generic(generics, &construct_trait_path);
    let register_all_immediate_dependencies = register_all_immediate_dependencies(ctors);
    let elim_ctor_idx = elim_ctor_idx(ctors);
    let introduction_rules = Macro {
        path: path_of_str("vec"),
        bang_token: <Token![!]>::default(),
        delimiter: MacroDelimiter::Bracket(Bracket::default()),
        tokens: introduction_rules(ctors).into_token_stream(),
    };
    let visit_deep = visit(ctors, &id("visit_deep"));
    let generated_tests = generated_tests(ident, &bounded_generics);

    let impl_path = Path {
        leading_colon: None,
        segments: iter::once(PathSegment {
            ident: ident.clone(),
            arguments: PathArguments::AngleBracketed(parameters),
        })
        .collect(),
    };
    if ctors.is_empty() {
        return quote! {
            impl #bounded_generics #construct_trait_path for #impl_path {
                #[inline]
                fn register_all_immediate_dependencies(
                    visited: &mut ::std::collections::BTreeSet<::pbt::reflection::Type>,
                    _sccs: &mut ::pbt::StronglyConnectedComponents,
                ) {
                    let _ = visited.insert(::pbt::reflection::type_of::<Self>());
                }

                #[inline]
                fn type_former() -> ::pbt::pbt::TypeFormer<Self> {
                    ::pbt::pbt::TypeFormer::Algebraic(::pbt::pbt::Algebraic {
                        introduction_rules: vec![],
                        elimination_rule: ::pbt::pbt::ElimFn::new(|uninhabited| match uninhabited {}),
                    })
                }

                #[inline]
                fn visit_deep<V>(&self) -> impl ::core::iter::Iterator<Item = V>
                where
                    V: ::pbt::pbt::Pbt,
                {
                    ::core::iter::empty()
                }
            }

            #generated_tests
        };
    }

    quote! {
        impl #bounded_generics #construct_trait_path for #impl_path {
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
            fn type_former() -> ::pbt::pbt::TypeFormer<Self> {
                ::pbt::pbt::TypeFormer::Algebraic(::pbt::pbt::Algebraic {
                    introduction_rules: #introduction_rules,
                    elimination_rule: ::pbt::pbt::ElimFn::new(|constructed| {
                        let mut fields = ::pbt::reflection::TermsOfVariousTypes::new();
                        let ctor_idx: usize = #elim_ctor_idx;
                        ::pbt::pbt::Decomposition {
                            // SAFETY: Case anaylsis above.
                            ctor_idx: unsafe { ::core::num::NonZero::new_unchecked(ctor_idx) },
                            fields,
                        }
                    }),
                })
            }

            #[inline]
            fn visit_deep<V>(&self) -> impl ::core::iter::Iterator<Item = V>
                where
                    V: ::pbt::pbt::Pbt,
                {
                ::pbt::pbt::visit_self(self).chain({
                    let iter: Box<dyn Iterator<Item = _>> = #visit_deep;
                    iter
                })
            }
        }

        #generated_tests
    }
}

#[cfg(feature = "generated-tests")]
/// Generate the opt-in regression test module emitted by `#[derive(Pbt)]`.
#[inline]
fn generated_tests(ident: &Ident, bounded_generics: &Generics) -> TokenStream {
    let test_mod_id = id(&format!("pbt_{ident}"));
    let test_path = Path {
        leading_colon: None,
        segments: [
            seg(id("super")),
            PathSegment {
                ident: ident.clone(),
                arguments: PathArguments::AngleBracketed(AngleBracketedGenericArguments {
                    colon2_token: None,
                    lt_token: <Token![<]>::default(),
                    args: bounded_generics
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
        #[cfg(test)]
        mod #test_mod_id {
            #[test]
            fn eta_expansion() {
                let () = ::pbt::pbt::check_eta_expansion::<#test_path>();
            }

            #[test]
            fn serialization_roundtrip() {
                let () = ::pbt::cache::check_roundtrip::<#test_path>();
            }
        }
    }
}

#[cfg(not(feature = "generated-tests"))]
/// Disable generated regression tests unless the macro crate's feature is enabled.
#[inline]
fn generated_tests(_ident: &Ident, _bounded_generics: &Generics) -> TokenStream {
    TokenStream::new()
}

/// Add a `Pbt` bound to each type parameter while preserving other parameters.
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

/// Convert generic parameters into generic arguments for referring to the derived type.
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

/// Return all type parameter names in these generics.
#[inline]
fn type_param_idents(generics: &Generics) -> Vec<String> {
    generics
        .params
        .iter()
        .filter_map(|param| match *param {
            GenericParam::Type(TypeParam { ref ident, .. }) => Some(ident.to_string()),
            GenericParam::Const(_) | GenericParam::Lifetime(_) => None,
        })
        .collect()
}

/// Generate statements that register every immediate field dependency.
#[inline]
fn register_all_immediate_dependencies(ctors: &[SourceCtor<'_>]) -> Block {
    Block {
        brace_token: Brace::default(),
        stmts: ctors
            .iter()
            .flat_map(|ctor| {
                ctor.fields.iter().map(|&Field { ref ty, .. }| {
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

/// Build the constructor metadata emitted by `type_former`.
#[inline]
fn introduction_rules(ctors: &[SourceCtor<'_>]) -> Punctuated<Expr, Token![,]> {
    ctors
        .iter()
        .map(|ctor| -> Expr {
            let path = &ctor.path;
            let ctor_fields = ctor.fields;
            Expr::Struct(ExprStruct {
                attrs: vec![],
                qself: None,
                path: Path {
                    leading_colon: Some(PathSep::default()),
                    segments: [
                        seg(id("pbt")),
                        seg(id("pbt")),
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
                        member: Member::Named(id("arbitrary")),
                        expr: Expr::Call(ExprCall {
                            attrs: vec![],
                            func: Box::new(Expr::Verbatim(
                                quote! { ::pbt::pbt::ArbitraryFn::new },
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
                                body: Box::new(Expr::Verbatim({
                                    let constructed = arbitrary_ctor(ctor);
                                    quote!({ Ok(Some(#constructed)) })
                                })),
                            }))
                            .collect(),
                        }),
                    },
                    FieldValue {
                        attrs: vec![],
                        colon_token: Some(<Token![:]>::default()),
                        member: Member::Named(id("call")),
                        expr: Expr::Call(ExprCall {
                            attrs: vec![],
                            func: Box::new(Expr::Verbatim(
                                quote! { ::pbt::pbt::CtorFn::new },
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
                                    let some = match *ctor_fields {
                                        Fields::Unit => Expr::Path(ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path: path.clone(),
                                        }),
                                        Fields::Unnamed(ref unnamed_fields) => Expr::Call(ExprCall {
                                            attrs: vec![],
                                            func: Box::new(Expr::Path(ExprPath {
                                                attrs: vec![],
                                                qself: None,
                                                path: path.clone(),
                                            })),
                                            paren_token: unnamed_fields.paren_token,
                                            args: unnamed_fields
                                                .unnamed
                                                .iter()
                                                .map(|&Field { ref ty, .. }| {
                                                    Expr::Verbatim(quote! {
                                                        terms.must_pop::<#ty>()
                                                    })
                                                })
                                                .collect(),
                                        }),
                                        Fields::Named(ref named_fields) => Expr::Struct(ExprStruct {
                                            attrs: vec![],
                                            qself: None,
                                            path: path.clone(),
                                            brace_token: named_fields.brace_token,
                                            fields: named_fields
                                                .named
                                                .iter()
                                                .enumerate()
                                                .map(
                                                    |(
                                                        i,
                                                        &Field {
                                                            ident: ref field_ident_opt,
                                                            ref ty,
                                                            ..
                                                        },
                                                    )| {
                                                        let field_ident = force_ident(field_ident_opt.as_ref(), i);
                                                        FieldValue {
                                                            attrs: vec![],
                                                            member: Member::Named(field_ident),
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
                                elems: ctor_fields
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

/// Build a direct constructor application for generated values.
#[inline]
fn arbitrary_ctor(ctor: &SourceCtor<'_>) -> TokenStream {
    let path = &ctor.path;
    match *ctor.fields {
        Fields::Unit => quote!(#path),
        Fields::Unnamed(ref unnamed_fields) => {
            let values = unnamed_fields.unnamed.iter().map(|field| {
                let ty = &field.ty;
                quote!(::pbt::pbt::arbitrary_field::<#ty>(&mut sizes, prng)?)
            });
            quote!(#path( #(#values),* ))
        }
        Fields::Named(ref named_fields) => {
            let values = named_fields.named.iter().enumerate().map(|(i, field)| {
                let field_ident = force_ident(field.ident.as_ref(), i);
                let ty = &field.ty;
                quote!(#field_ident: ::pbt::pbt::arbitrary_field::<#ty>(&mut sizes, prng)?)
            });
            quote!(#path { #(#values),* })
        }
    }
}

/// Build the eliminator match that records constructor indices and fields.
#[inline]
fn elim_ctor_idx(ctors: &[SourceCtor<'_>]) -> ExprMatch {
    ExprMatch {
        attrs: vec![],
        match_token: <Token![match]>::default(),
        expr: Box::new(Expr::Verbatim(quote! { constructed })),
        brace_token: Brace::default(),
        arms: ctors
            .iter()
            .enumerate()
            .map(|(zero_based_index, ctor)| {
                let path = &ctor.path;
                let ctor_fields = ctor.fields;
                // SAFETY: Adding 1.
                let ctor_index = unsafe {
                    NonZero::new_unchecked(
                        #[expect(clippy::expect_used, reason = "extremely unlikely")]
                        zero_based_index
                            .checked_add(1)
                            .expect("internal `pbt` error: more than `usize::MAX` constructors"),
                    )
                };
                Arm {
                    attrs: vec![],
                    pat: match *ctor_fields {
                        Fields::Unit => Pat::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                        }),
                        Fields::Named(ref named_fields) => Pat::Struct(PatStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            brace_token: named_fields.brace_token,
                            fields: named_fields
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
                        Fields::Unnamed(ref unnamed_fields) => Pat::TupleStruct(PatTupleStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            paren_token: unnamed_fields.paren_token,
                            elems: unnamed_fields
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
                            stmts: ctor_fields
                                .iter()
                                .enumerate()
                                .rev()
                                .map(
                                    |(
                                        i,
                                        &Field {
                                            ident: ref field_ident_opt,
                                            ref ty,
                                            ..
                                        },
                                    )| {
                                        let field_ident = force_ident(field_ident_opt.as_ref(), i);
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
                                                    fields.push::<#ty>(#field_ident)
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
                                            &ctor_index.to_string(),
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

/// Build the `visit_deep` match that chains recursive field visitors.
#[inline]
fn visit(ctors: &[SourceCtor<'_>], visit_fn: &Ident) -> ExprMatch {
    ExprMatch {
        attrs: vec![],
        match_token: <Token![match]>::default(),
        expr: Box::new(Expr::Verbatim(quote! { self })),
        brace_token: Brace::default(),
        arms: ctors
            .iter()
            .map(|ctor| {
                let path = &ctor.path;
                let ctor_fields = ctor.fields;
                Arm {
                    attrs: vec![],
                    pat: match *ctor_fields {
                        Fields::Unit => Pat::Path(ExprPath {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                        }),
                        Fields::Named(ref named_fields) => Pat::Struct(PatStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            brace_token: named_fields.brace_token,
                            fields: named_fields
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
                        Fields::Unnamed(ref unnamed_fields) => Pat::TupleStruct(PatTupleStruct {
                            attrs: vec![],
                            qself: None,
                            path: path.clone(),
                            paren_token: unnamed_fields.paren_token,
                            elems: unnamed_fields
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
                        let iter = ctor_fields.iter().enumerate().fold(
                            Expr::Verbatim(quote! { ::core::iter::empty() }),
                            |acc,
                             (
                                i,
                                &Field {
                                    ident: ref field_ident_opt,
                                    ..
                                },
                            )| {
                                let field_ident = force_ident(field_ident_opt.as_ref(), i);
                                Expr::MethodCall(ExprMethodCall {
                                    attrs: vec![],
                                    receiver: Box::new(Expr::MethodCall(ExprMethodCall {
                                        attrs: vec![],
                                        receiver: Box::new(Expr::Path(ExprPath {
                                            attrs: vec![],
                                            qself: None,
                                            path: Path {
                                                leading_colon: None,
                                                segments: iter::once(seg(field_ident)).collect(),
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
                }
            })
            .collect(),
    }
}

/// Wrap an identifier as a single-segment path.
#[inline]
fn path_of_id(ident: Ident) -> Path {
    Path {
        leading_colon: None,
        segments: iter::once(seg(ident)).collect(),
    }
}

/// Build a single-segment path from a string.
#[inline]
fn path_of_str(str: &str) -> Path {
    path_of_id(id(str))
}

/// Build an identifier at the call site.
#[inline]
fn id(str: &str) -> Ident {
    Ident::new(str, Span::call_site())
}

/// Build a path segment without generic arguments.
#[inline]
fn seg(ident: Ident) -> PathSegment {
    PathSegment {
        ident,
        arguments: PathArguments::None,
    }
}

/// Return a field identifier or synthesize one for tuple fields.
#[inline]
fn force_ident(maybe_id: Option<&Ident>, index: usize) -> Ident {
    maybe_id.map_or_else(|| id(&format!("_{index}")), Clone::clone)
}
