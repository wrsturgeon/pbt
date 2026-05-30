use {
    crate::{
        Pbt,
        fields::Store,
        reflection::{
            BucketOps, Constructor, Erased, Parts, bucket_ops_of, constructors_of,
            register_globally, shrink_literal,
        },
    },
    core::{any::TypeId, mem, ptr},
};

pub struct EachField {
    /// How many shrinking steps are we allowed to take,
    /// summed over *all* fields (from here down)?
    leash_length: usize,
    recurse: EachFieldRecursively,
}

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
    cache: Vec<Erased>,
    iterator: Box<dyn Iterator<Item = ptr::NonNull<Erased>>>,
    original: ptr::NonNull<Erased>,
    /// The type being shrunk.
    ty: TypeId,
}

impl EachField {
    #[inline]
    fn new(fields: Store) -> Self {
        Self {
            leash_length: 1,
            recurse: EachFieldRecursively::new(fields),
        }
    }
}

impl Iterator for EachField {
    type Item = Store;

    #[inline]
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

pub(crate) fn candidates<T>(t: T) -> Box<dyn Iterator<Item = T>>
where
    T: Pbt,
{
    let () = register_globally::<T>();
    if let Some(shrink_literal) = shrink_literal(t.clone()) {
        return shrink_literal;
    }

    let ty = TypeId::of::<T>();

    let Parts {
        fields,
        variant_index,
    } = t.deconstruct();

    // First, find all sub-terms of type `Self` and try them at the top level:
    let subterms_of_type_self = fields.clone().visit::<T>();

    // Then, try all variants smaller than the original variant
    // using all sections of available fields necessary for each:
    let ctors: Vec<Constructor> = constructors_of(ty).algebraic().to_vec();
    let smaller_variants: Vec<T> = ctors
        .into_iter()
        .take_while(move |ctor| ctor.index != variant_index)
        .flat_map(|ctor| {
            fields
                .clone()
                .sections(ctor.field_types().clone())
                .map(move |field_section| {
                    T::construct(Parts {
                        fields: field_section,
                        variant_index: ctor.index,
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

#[cfg(test)]
mod tests {
    use {super::*, pretty_assertions::assert_eq};

    #[test]
    fn shrink_triple() {
        let v: Vec<usize> = vec![2, 2, 2];
        let shrunk: Vec<Vec<usize>> = candidates(v).collect();
        let expected: Vec<Vec<usize>> = vec![
            vec![2, 2],
            vec![2],
            vec![],
            vec![],
            vec![0],
            vec![2, 1],
            vec![0],
            vec![1],
            vec![2, 2],
            vec![0, 0],
            vec![1],
            vec![2],
            vec![1, 0],
            vec![0, 1],
            vec![2],
            vec![1, 0, 0],
            vec![1, 1],
            vec![0, 2],
            vec![1, 0],
            vec![1, 0, 1],
            vec![1, 2],
            vec![2, 0],
            vec![1, 1],
            vec![1, 0, 2],
            vec![2, 0, 0],
            vec![2, 1],
            vec![1, 2],
            vec![1, 1, 0],
            vec![2, 0, 1],
            vec![2, 2],
            vec![2, 0],
            vec![1, 1, 1],
            vec![2, 0, 2],
            vec![2, 1, 0],
            vec![2, 1],
            vec![1, 1, 2],
            vec![1, 2, 0],
            vec![2, 1, 1],
            vec![2, 2],
            vec![2, 2, 0],
            vec![1, 2, 1],
            vec![2, 1, 2],
            vec![2, 2, 1],
            vec![1, 2, 2],
        ];
        assert_eq!(shrunk, expected);
    }
}
