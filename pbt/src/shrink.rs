use {
    crate::{
        pbt::{Decomposition, Pbt},
        reflection::{AlgebraicTypeFormer, Erased, PrecomputedTypeFormer, info},
    },
    core::mem,
};

// TODO: Take a reference instead of a moved value.
/// Iterate over values that are "smaller" than this one in some sense.
/// This iterator is designed to cut about half the remaining "size" of the type
/// on the first go, then to cut only about a quarter, then only an eighth, etc.,
/// until they almost reach (but do not equal) the original term.
#[inline]
pub fn shrink<T>(t: T) -> Box<dyn Iterator<Item = T>>
where
    T: Pbt,
{
    let info = info::<T>();
    let AlgebraicTypeFormer {
        all_constructors: ref erased_ctors,
        eliminator: erased_eliminator,
        ..
    } = *match info.type_former {
        PrecomputedTypeFormer::Algebraic(ref alg) => alg,
        PrecomputedTypeFormer::Literal {
            shrink: erased_shrink,
            ..
        } => {
            // SAFETY: Undoing an earlier `transmute`.
            let shrink = unsafe {
                mem::transmute::<
                    fn(Erased) -> Box<dyn Iterator<Item = Erased>>,
                    fn(T) -> Box<dyn Iterator<Item = T>>,
                >(erased_shrink)
            };
            return shrink(t);
        }
    };
    let ctors = erased_ctors.clone();

    // SAFETY: Undoing an earlier `erase`.
    let eliminator = unsafe { erased_eliminator.unerase::<T>() };
    let Decomposition {
        ctor_idx,
        fields: orig_fields,
    } = eliminator(t.clone());
    // SAFETY: By the correct implementation of `eliminator`
    // (i.e., by macro logic plus the few implementations in this crate).
    #[expect(clippy::multiple_unsafe_ops_per_block, reason = "logically grouped")]
    let (erased_orig_ctor_fn, orig_ctor_deps) =
        unsafe { ctors.get_unchecked(ctor_idx.get().unchecked_sub(1)) }.clone();
    // SAFETY: Undoing an earlier `erase`.
    let orig_ctor_fn = unsafe { erased_orig_ctor_fn.unerase::<T>() };
    // Visit all terms of type `Self` (deeply) and try them all as toplevel solutions:
    let nested_selves = t
        .visit_deep::<T>()
        .skip(1) // skip `t` itself
        .collect::<Vec<_>>(); // need to collect b/c `t` is local :(

    let shrink_fields = orig_fields
        .clone()
        .shrink()
        .filter_map(move |mut shrunk_fields| orig_ctor_fn(&mut shrunk_fields));

    // Try all other constructors whose field multisets are subsets of `t`'s fields:
    // (It's fine that constructors are unsorted, since success will effectively restart,
    // and that's probably just as efficient as eagerly sorting and/or
    // storing the constructors in sorted order but then multiplexing indices.)
    // TODO: Benchmark the above -- but still, not a big deal.
    let try_smaller_ctors = ctors
        .into_iter()
        .filter(move |&(_, ref deps)| {
            let ctor_info = &deps.constructor;
            // We can reuse fields iff the other constructor's fields are a
            // sub(multi)set of this constructor's fields;
            // otherwise, we'd have to generate new fields,
            // and the whole resulting value might be larger than this one:
            ctor_info
                .immediate
                .is_subset_of(&orig_ctor_deps.constructor.immediate)
                .is_some_and(|strict| {
                    strict || {
                        // If the two constructors' fields were *precisely* equal
                        // (still technically a subset, just not a strict one),
                        // then we need to tiebreak a potential loop:
                        ctor_info.index < orig_ctor_deps.constructor.index
                    }
                })
            // TODO: should we do this for *all* (deep) terms
            // or just immediate toplevel fields?
        })
        .filter_map(move |(erased_ctor_fn, _)| {
            // SAFETY: Undoing an earlier `erase`.
            let ctor_fn = unsafe { erased_ctor_fn.unerase::<T>() };
            // TODO: iterate over sections if there would be fields left over;
            // right now, we're effectively only taking the first N and dropping the rest
            ctor_fn(&mut orig_fields.clone())
        });

    Box::new(
        nested_selves
            .into_iter()
            .chain(try_smaller_ctors)
            .chain(shrink_fields),
    )
}
