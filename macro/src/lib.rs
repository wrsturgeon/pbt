//! Property-based testing plus `#[derive(..)]`, no-std, automatic edge cases, and exhaustive breadth-first search over arbitrary types.

use quote::{ToTokens as _, format_ident, quote};

/// The maximum size of a tuple for which we implement all relevant traits.
/// `core::fmt::Debug` is implemented only up to 12, so that's a reasonable choice.
const MAX_TUPLE_SIZE: usize = 12;

#[proc_macro]
#[expect(
    clippy::cognitive_complexity,
    clippy::too_many_lines,
    reason = "just formulaic `quote` expansion"
)]
pub fn impl_non_empty_tuples(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let mut ts = proc_macro2::TokenStream::new();
    for tuple_size in 1..=MAX_TUPLE_SIZE {
        #[expect(clippy::arithmetic_side_effects, reason = "Starts at 1 (above).")]
        let final_index = tuple_size - 1;

        // Iterators over identifiers to use in `quote` later:
        let ty: Vec<_> = (0..tuple_size).map(|i| format_ident!("T{i}")).collect();
        let val: Vec<_> = (0..tuple_size).map(|i| format_ident!("t{i}")).collect();

        let final_val = format_ident!("t{final_index}");
        let final_ty = format_ident!("T{final_index}");

        let nested_values = {
            let mut acc = quote! { #final_val };
            for i in (0..final_index).rev() {
                let val = format_ident!("t{i}");
                acc = quote! { (#val, #acc) };
            }
            acc
        };

        let nested_types = {
            let mut acc = quote! { #final_ty };
            for i in (0..final_index).rev() {
                let ty = format_ident!("T{i}");
                acc = quote! { (#ty, #acc) };
            }
            acc
        };

        let nested_edge_case_types = {
            let mut acc = quote! { MaybeIterator<MakeEdgeCases<#final_ty>> };
            for i in (0..final_index).rev() {
                let ty = format_ident!("T{i}");
                acc = quote! { (CachingIterator<MakeEdgeCases<#ty>>, #acc) };
            }
            acc
        };

        let nested_exhaust_types = {
            let mut acc = quote! { MaybeIterator<MakeExhaust<#final_ty>> };
            for i in (0..final_index).rev() {
                let ty = format_ident!("T{i}");
                acc = quote! { (CachingIterator<MakeExhaust<#ty>>, #acc) };
            }
            acc
        };

        let nested_iterator_values = {
            let mut acc = quote! { MaybeIterator::Inactive };
            for _ in 0..final_index {
                acc = quote! { (CachingIterator::Inactive, #acc) };
            }
            acc
        };

        let () = quote! {

            impl<#(#ty: AstSize),*> AstSize for (#(#ty,)*) {
                const MAX_AST_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = {
                    let acc = MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0_usize)));
                    #(let acc = acc.cartesian_product_with_self(&<#ty>::MAX_AST_SIZE);)*
                    acc
                };
                const MAX_EXPECTED_AST_SIZE: MaybeDecidable<Max<f32>> = {
                    let mut acc = MaybeDecidable::Decidable(Max::Finite(0_f32));
                    #(let acc = acc.cartesian_product_with_self(&<#ty>::MAX_EXPECTED_AST_SIZE);)*
                    acc
                };
                #[inline]
                fn ast_size(&self) -> MaybeOverflow<usize> {
                    let (#(ref #val,)*) = *self;
                    let acc = MaybeOverflow::Contained(0_usize);
                    #(let acc = acc.plus_self(#val.ast_size());)*
                    acc
                }
            }

            impl<#(#ty: ValueSize),*> ValueSize for (#(#ty,)*) {
                const MAX_VALUE_SIZE: MaybeDecidable<Max<MaybeOverflow<usize>>> = {
                    let acc = MaybeDecidable::Decidable(Max::Finite(MaybeOverflow::Contained(0_usize)));
                    #(let acc = acc.cartesian_product_with_self(&<#ty>::MAX_VALUE_SIZE);)*
                    acc
                };
                #[inline]
                fn value_size(&self) -> MaybeOverflow<usize> {
                    let (#(ref #val,)*) = *self;
                    let acc = MaybeOverflow::Contained(0_usize);
                    #(let acc = acc.plus_self(#val.value_size());)*
                    acc
                }
            }

            impl<#(#ty: Clone + EdgeCases),*> EdgeCases for (#(#ty,)*) {
                type EdgeCases = iter::Map<FlattenNestedIterator<#nested_edge_case_types>, fn(#nested_types) -> Self>;
                #[inline]
                fn edge_cases() -> Self::EdgeCases {
                    Iterator::map(
                        FlattenNestedIterator {
                            total_size: 0,
                            nested_iterator: #nested_iterator_values,
                        },
                        move |#nested_values| (#(#val,)*),
                    )
                }
            }

            impl<#(#ty: Clone + Exhaust),*> Exhaust for (#(#ty,)*) {
                type Exhaust = iter::Map<FlattenNestedIterator<#nested_exhaust_types>, fn(#nested_types) -> Self>;
                #[inline]
                fn exhaust(value_size: usize) -> Result<Self::Exhaust, error::UnreachableSize> {
                    if const { Self::MAX_VALUE_SIZE.at_most() } < &Max::Finite(MaybeOverflow::Contained(value_size)) {
                        return Err(error::UnreachableSize);
                    } else {
                        Ok(Iterator::map(
                            FlattenNestedIterator {
                                total_size: value_size,
                                nested_iterator: #nested_iterator_values,
                            },
                            move |#nested_values| (#(#val,)*),
                        ))
                    }
                }
            }

            impl<#(#ty: Pseudorandom),*> Pseudorandom for (#(#ty,)*) {
                #[inline]
                fn pseudorandom<Rng: RngCore>(
                    expected_ast_size: f32,
                    rng: &mut Rng,
                ) -> Result<Self, error::Uninstantiable> {
                    let crate::impls::tuples::FullAndPartialFields {
                        n_full_fields,
                        sum_of_partial_sizes,
                    } = crate::impls::tuples::full_and_partial_fields(
                        expected_ast_size * const { 1. / (#tuple_size as f32) },
                        &[#(#ty::MAX_EXPECTED_AST_SIZE,)*],
                    );
                    // Really, we should iterate until we reach a fixed point,
                    // but this will work very well as a good-enough approximation,
                    // and the throughput trade-off is a no-brainer.
                    let size_per_element = (expected_ast_size - sum_of_partial_sizes) / (n_full_fields as f32);
                    Ok((#(#ty::pseudorandom(size_per_element, rng)?,)*))
                }
            }

        }
        .to_tokens(&mut ts);
    }
    ts.into()
}
