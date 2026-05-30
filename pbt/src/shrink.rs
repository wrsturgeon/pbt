use {
    crate::{
        Pbt,
        fields::Store,
        reflection::{BucketOps, Constructor, Erased, Parts, bucket_ops_of, constructors_of},
    },
    core::{any::TypeId, ptr},
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

    #[inline]
    fn next_with_leash(&mut self, leash_length: usize) -> Option<Store> {
        let Some((ref mut recurse, ref mut shrinking_cache)) = self.recurse else {
            if leash_length == 0 && self.index == 0 {
                self.index = 1;
                return Some(Store::new());
            }
            return None;
        };
        loop {
            let remaining_leash = leash_length.checked_sub(self.index)?;
            let head = shrinking_cache.get(self.index)?;
            if let Some(mut next) = recurse.next_with_leash(remaining_leash) {
                let () =
                    next.push_erased(shrinking_cache.ty, (shrinking_cache.bucket_ops.clone)(head));
                return Some(next);
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
    #[inline]
    fn get(&mut self, index: usize) -> Option<ptr::NonNull<Erased>> {
        // We only ever extend this cache by one element at a time,
        // so this dumb retry loop is not only fine but optimal:
        loop {
            if let Some(cached_ref) = (self.bucket_ops.get)(&mut self.cache, index) {
                return Some(cached_ref);
            }
            let next = self.iterator.next()?;
            let () = (self.bucket_ops.push)(&mut self.cache, next);
        }
    }

    #[inline]
    fn new(ty: TypeId, erased_boxed: ptr::NonNull<Erased>) -> Self {
        let bucket_ops = bucket_ops_of(ty);
        let mut cache = vec![];
        let () = (bucket_ops.push)(&mut cache, (bucket_ops.clone)(erased_boxed));
        Self {
            bucket_ops,
            cache,
            iterator: (bucket_ops.shrink)(erased_boxed),
            ty,
        }
    }
}

impl Iterator for EachField {
    type Item = Store;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(next) = self.recurse.next_with_leash(self.leash_length) {
            return Some(next);
        }
        let next = self.recurse.next_with_leash(self.leash_length + 1)?;
        self.leash_length += 1;
        Some(next)
    }
}

pub(crate) fn candidates<T>(t: T) -> impl Iterator<Item = T>
where
    T: Pbt,
{
    let ty = TypeId::of::<T>();

    let Parts {
        fields,
        variant_index,
    } = t.deconstruct();

    // First, find all sub-terms of type `Self` and try them at the top level:
    let subterms_of_type_self = fields.clone().visit::<T>();

    // Then, recurse on all fields, in some way TBD that's "fair":
    let shrink_each_field = EachField::new(fields.clone()).map(move |shrunk_fields| {
        T::construct(Parts {
            fields: shrunk_fields,
            variant_index,
        })
    });

    // Then, try all variants smaller than the original variant
    // using all sections of available fields necessary for each:
    let ctors: Vec<Constructor<Erased>> = constructors_of(ty).iter().cloned().collect();
    let smaller_variants = ctors
        .into_iter()
        .take_while(move |ctor| ctor.index != variant_index)
        .flat_map(move |ctor| {
            fields
                .clone()
                .sections(ctor.field_types().clone())
                .map(move |field_section| {
                    T::construct(Parts {
                        fields: field_section,
                        variant_index: ctor.index,
                    })
                })
        });

    subterms_of_type_self
        .chain(smaller_variants)
        .chain(shrink_each_field)
}

#[cfg(test)]
mod tests {
    // use {super::*, pretty_assertions::assert_eq};

    // TODO: re-enable
    /*
    #[test]
    fn shrink_10_10_10() {
        let v: Vec<usize> = vec![10, 10, 10];
        let shrunk: Vec<Vec<usize>> = candidates(v).collect();
        let expected: Vec<Vec<usize>> = vec![
            // left empty to see what's produced when this test fails
        ];
        assert_eq!(shrunk, expected);
    }
    */
}
