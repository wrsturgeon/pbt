//! Shrinking candidates for witnesses found by property-based search.

use {
    crate::{
        Pbt,
        fields::Store,
        persist,
        reflection::{BucketOps, Constructors, Erased, Parts, bucket_ops_of, constructors_of},
    },
    alloc::sync::Arc,
    core::{any::TypeId, mem, ptr},
};

/// Iterate over all combinations produced by shrinking this constructor's fields.
pub struct EachField {
    /// How many shrinking steps are we allowed to take,
    /// summed over *all* fields (from here down)?
    leash_length: usize,
    /// Recursive iterator over the fields being shrunk.
    recurse: EachFieldRecursively,
}

/// Iterate over field shrink combinations with a fixed total shrink budget.
pub struct EachFieldRecursively {
    /// How many shrinking steps should this field take?
    index: usize,
    /// Recurse on remaining fields, if any.
    recurse: Option<(Box<Self>, ShrinkingCache)>,
}

/// Lazily extended cache of shrinking steps.
pub struct ShrinkingCache {
    /// Function pointers performing operations on vectors of some type.
    bucket_ops: BucketOps<Erased>, // <-- TODO: duplicated across `Vec<ShrinkingCache>`
    /// Already-computed shrinking candidates.
    cache: Vec<Erased>,
    /// The underlying shrinking iterator, extended lazily into `cache`.
    iterator: Box<dyn Iterator<Item = ptr::NonNull<Erased>>>,
    /// A clone of the original value, yielded after all proper shrinks.
    original: ptr::NonNull<Erased>,
    /// The type being shrunk.
    ty: TypeId,
}

impl EachField {
    /// Shrink fields from this store one shrinking step at a time.
    #[inline]
    fn new(fields: Store) -> Self {
        Self {
            leash_length: 0,
            recurse: EachFieldRecursively::new(fields),
        }
    }
}

impl Iterator for EachField {
    type Item = Store;

    #[inline]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "A process cannot enumerate enough shrink combinations to overflow `usize`."
    )]
    fn next(&mut self) -> Option<Self::Item> {
        'exclude_orig: loop {
            let Some((mut next, orig)) = self.recurse.next_with_leash(self.leash_length) else {
                break 'exclude_orig;
            };
            if !orig {
                return Some(next);
            }
            let () = next.drop_unused();
        }
        let () = self.recurse.rewind();
        self.leash_length += 1;
        loop {
            let (mut next, orig) = self.recurse.next_with_leash(self.leash_length)?;
            if !orig {
                return Some(next);
            }
            let () = next.drop_unused();
        }
    }
}

impl EachFieldRecursively {
    /// Prepare recursive shrinking for the next erased field in this store.
    #[inline]
    fn new(mut fields: Store) -> Self {
        let recurse = fields.pop_erased().map(move |(ty, erased_boxed)| {
            (
                Box::new(Self::new(fields)),
                ShrinkingCache::new(ty, erased_boxed),
            )
        });
        Self { index: 0, recurse }
    }

    /// Yield the next item with at most some total number of shrinking steps.
    ///
    /// The `bool` keeps track of whether all retured fields so far are originals,
    /// so we can exclude the original input at the top level.
    #[inline]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "A process cannot enumerate enough shrink combinations to overflow `usize`."
    )]
    fn next_with_leash(&mut self, leash_length: usize) -> Option<(Store, bool)> {
        let Some((ref mut recurse, ref mut shrinking_cache)) = self.recurse else {
            if leash_length == 0 && self.index == 0 {
                self.index = 1;
                return Some((Store::new(), true));
            }
            return None;
        };
        loop {
            let remaining_leash = leash_length.checked_sub(self.index)?;
            let (head, orig) = shrinking_cache.get(self.index)?;
            if let Some((mut next, all_orig)) = recurse.next_with_leash(remaining_leash) {
                let () =
                    next.push_erased(shrinking_cache.ty, (shrinking_cache.bucket_ops.clone)(head));
                return Some((next, all_orig && orig));
            }
            self.index += 1;
            let () = recurse.rewind();
        }
    }

    #[inline]
    /// Rewind this field-combination iterator back to the beginning.
    fn rewind(&mut self) {
        self.index = 0;
        if let Some((ref mut recurse, _)) = self.recurse {
            let () = recurse.rewind();
        }
    }
}

impl ShrinkingCache {
    /// Return an erased *reference* (*not* a `Box`) to the nth shrinking candidate.
    ///
    /// The `bool` indicates whether this reference is
    /// the *original* input (not a shrunk version thereof).
    #[inline]
    fn get(&mut self, index: usize) -> Option<(ptr::NonNull<Erased>, bool)> {
        // We only ever extend this cache by one element at a time,
        // so this dumb retry loop is not only fine but optimal:
        loop {
            if let Some(cached_ref) = (self.bucket_ops.get)(&mut self.cache, index) {
                return Some((cached_ref, false));
            }
            let Some(next) = self.iterator.next() else {
                return (index == self.cache.len()).then_some((self.original, true));
            };
            let () = (self.bucket_ops.push)(&mut self.cache, next);
        }
    }

    /// Initialize a lazily-filled shrinking cache for one erased value.
    #[inline]
    fn new(ty: TypeId, erased_boxed: ptr::NonNull<Erased>) -> Self {
        let bucket_ops = bucket_ops_of(ty);
        // Clone `erased_boxed` before moving it into `bucket_ops.shrink`:
        let original = (bucket_ops.clone)(erased_boxed);
        let iterator = (bucket_ops.shrink)(erased_boxed);
        Self {
            bucket_ops,
            cache: (bucket_ops.empty)(),
            iterator,
            original,
            ty,
        }
    }
}

impl Drop for ShrinkingCache {
    #[inline]
    fn drop(&mut self) {
        let () = (self.bucket_ops.drop)(self.original);
        let () = (self.bucket_ops.drop_vec)(mem::take(&mut self.cache));
    }
}

/// Iterate over all shrinking candidates for a witness.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn candidates<T>(t: T) -> Box<dyn Iterator<Item = T>>
where
    T: Pbt,
{
    let ty = TypeId::of::<T>();

    let ctors = match constructors_of(ty) {
        Constructors::Algebraic(ref algebraic) => Arc::clone(algebraic),
        Constructors::Literal { shrink, .. } => {
            // SAFETY: Invariant. Extremely dangerous.
            let typed_shrink = unsafe {
                mem::transmute::<
                    fn(Erased) -> Box<dyn Iterator<Item = Erased>>,
                    fn(T) -> Box<dyn Iterator<Item = T>>,
                >(shrink)
            };
            return typed_shrink(t);
        }
    };

    let Parts {
        fields,
        variant_index,
    } = t.deconstruct();
    let index =
        variant_index.expect("INTERNAL ERROR (`pbt`): algebraic type without a variant index");

    // First, find all sub-terms of type `Self` and try them at the top level:
    let subterms_of_type_self = fields.clone().visit::<T>();

    // Then, try all variants smaller than the original variant
    // using all sections of available fields necessary for each:
    let smaller_variants: Vec<T> = ctors
        .iter()
        .take_while(move |ctor| ctor.index != index)
        .flat_map(|ctor| {
            fields
                .clone()
                .sections(ctor.field_types().clone())
                .map(move |field_section| {
                    T::construct(Parts {
                        fields: field_section,
                        variant_index: Some(ctor.index),
                    })
                })
        })
        .collect();

    // Then, recurse on all fields:
    let shrink_each_field = EachField::new(fields).map(move |shrunk_fields| {
        T::construct(Parts {
            fields: shrunk_fields,
            variant_index,
        })
    });

    Box::new(
        subterms_of_type_self
            .chain(smaller_variants)
            .chain(shrink_each_field),
    )
}

/// Find an approximately-global minimum for a given property,
/// starting from a witness that is probably far larger than necessary.
#[inline]
pub(crate) fn to_minimal_witness<T, Property, Proof>(
    property: &Property,
    mut best_yet: T,
    mut proof: Proof,
) -> (T, Proof)
where
    Property: Fn(&T) -> Option<Proof>,
    T: Pbt,
{
    'giant_leaps: loop {
        for candidate in candidates::<T>(best_yet.clone()) {
            if let Some(next_proof) = property(&candidate) {
                best_yet = candidate;
                proof = next_proof;
                continue 'giant_leaps;
            }
        }
        let () = persist::witness(&best_yet);
        return (best_yet, proof);
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::reflection::register_globally, pretty_assertions::assert_eq};

    #[test]
    fn shrink_triple() {
        let () = register_globally::<Vec<usize>>();
        let v: Vec<usize> = vec![2, 2, 2];
        let mut iter_candidates = candidates(v);
        for expected in [
            vec![2, 2],
            vec![2],
            vec![],
            vec![],
            vec![2, 0],
            vec![0],
            vec![2, 1],
            vec![0],
            vec![1],
            vec![2, 2],
            vec![0, 0],
            vec![1],
            vec![2],
            vec![0, 0],
            vec![0, 1],
            vec![2],
            vec![1, 0],
            vec![0, 1],
            vec![0, 2],
            vec![0, 0, 0],
            vec![1, 1],
            vec![0, 2],
            vec![1, 0],
            vec![0, 0, 1],
            vec![1, 2],
            vec![2, 0],
            vec![1, 1],
            vec![0, 0, 2],
            vec![1, 0, 0],
            vec![2, 1],
            vec![1, 2],
            vec![0, 1, 0],
            vec![1, 0, 1],
            vec![2, 2],
            vec![2, 0],
            vec![0, 1, 1],
            vec![1, 0, 2],
            vec![2, 0, 0],
            vec![2, 1],
            vec![0, 1, 2],
            vec![1, 1, 0],
            vec![2, 0, 1],
            vec![2, 2],
            vec![0, 2, 0],
            vec![1, 1, 1],
            vec![2, 0, 2],
            vec![2, 1, 0],
            vec![0, 2, 1],
            vec![1, 1, 2],
            vec![1, 2, 0],
            vec![2, 1, 1],
            vec![0, 2, 2],
            vec![2, 2, 0],
            vec![1, 2, 1],
            vec![2, 1, 2],
            vec![2, 2, 1],
            vec![1, 2, 2],
        ] {
            assert_eq!(iter_candidates.next(), Some(expected));
        }
        assert_eq!(iter_candidates.next(), None);
    }
}
