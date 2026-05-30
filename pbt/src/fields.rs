//! Logic for generating and/or storing
//! fields to be used on a given constructor.

use {
    crate::{
        Pbt,
        hash::map,
        multiset::Multiset,
        reflection::{BucketOps, Erased, bucket_ops_of, is_literal, register_globally},
        size::{self, Size},
        swarm::Swarm,
    },
    ahash::HashMap,
    core::{any::TypeId, mem, ptr},
    std::collections::hash_map,
    wyrand::WyRand,
};

/// Logic for generating and/or storing
/// fields to be used on a given constructor.
///
/// Note that this unifies two cases:
/// generation, in which we want fields on demand with maximum throughput,
/// and shrinking, in which we want to reuse existing fields.
pub trait Fields {
    /// Retrieve and/or generate a term of type T.
    fn field<T>(&mut self) -> T
    where
        T: Pbt;
}

/// Fields are not stored ahead of time;
/// instead, their sizes are stored in an iterator,
/// and all fields are produced just in time.
#[non_exhaustive]
pub(crate) struct Lazy<'prng, 'swarm> {
    /// Pseudorandom number generator.
    ///
    /// This is inside `Lazy` and not a function argument
    /// because shrinking (existing fields) doesn't need a PRNG.
    pub(crate) prng: &'prng mut WyRand,
    /// A lazy partition over sizes, tuned to match
    /// the number of inductive types among the fields to generate.
    pub(crate) sizes: size::Partition,
    /// A masked view into this type's constructors,
    /// partitioned into potential leaves and loops.
    pub(crate) swarm: &'swarm Swarm,
}

/// Iterate over all possible subsets and orderings
/// using these stored fields to create a sub-store
/// containing a requested multiset of types.
#[non_exhaustive]
struct Sections {
    /// One type at a time, index over all stored terms of that type.
    index: usize,
    /// One type at a time, index over all stored terms of that type.
    maybe_ty: Option<TypeId>,
    /// After removing one term of the currently focused type,
    /// recurse with that term removed from both the store and the multiset.
    recurse: Option<(ptr::NonNull<Erased>, Box<Self>)>,
    /// The desired output multiset of types.
    requirements: Option<Multiset<TypeId>>,
    /// The store of which we're iterating over sections.
    store: Store,
}

/// A collection of fields of arbitrary/mixed types.
/// Fields are known and returned if present;
/// unknown fields are newly generated leaves.
#[non_exhaustive]
pub struct Store {
    /// A map from type IDs to erased vectors
    /// whose elements match the associated type.
    store: HashMap<TypeId, Vec<Erased>>,
}

/// Visit all sub-terms of an arbitrary type within a `Store`.
#[non_exhaustive]
struct Visitor<T> {
    /// Function pointers performing operations on vectors of `self.ty`.
    bucket_ops: BucketOps<Erased>,
    /// All immediate sub-terms of type `T`.
    matches: Vec<T>,
    /// All immediate sub-terms of type `self.ty`.
    queue: Option<Vec<Erased>>,
    /// Recurse on each field.
    recurse: Option<Box<Self>>,
    /// The original store we're visiting.
    store: Store,
    /// The type on which we're currently recursing.
    ty: TypeId,
}

impl Fields for Lazy<'_, '_> {
    #[inline]
    fn field<T>(&mut self) -> T
    where
        T: Pbt,
    {
        let size = if self.swarm.is_inductive::<T>() {
            // SAFETY: `Partition::next` always returns `Some(_)`,
            // since it returns endless zeros after its assigned cardinality.
            unsafe { self.sizes.next().unwrap_unchecked() }
        } else {
            Size::zero()
        };
        self.swarm.arbitrary(size, self.prng)
    }
}

impl Sections {
    /// Iterate over all possible subsets and orderings
    /// using these stored fields to create a sub-store
    /// containing a requested multiset of types.
    #[inline]
    fn new(store: Store, mut requirements: Multiset<TypeId>) -> Self {
        Self {
            index: 0,
            maybe_ty: requirements.pop(),
            recurse: None,
            requirements: Some(requirements),
            store,
        }
    }
}

impl Drop for Sections {
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    fn drop(&mut self) {
        let () = self.store.drop_unused();
        if let Some((head, _)) = self.recurse.take() {
            let ty = self
                .maybe_ty
                .expect("INTERNAL ERROR (`pbt`): unused `Sections` head without a type");
            let () = (bucket_ops_of(ty).drop)(head);
        }
    }
}

impl Iterator for Sections {
    type Item = Store;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let requirements = self.requirements.as_ref()?;
            let Some(ty) = self.maybe_ty else {
                // No requirements, so we should return the empty store:
                self.requirements = None; // <-- "don't return another after this" (see above)
                return Some(Store::new());
            };
            let bucket_ops = bucket_ops_of(ty);

            if let Some((ref head, ref mut recurse)) = self.recurse {
                if let Some(mut tail) = recurse.next() {
                    let v: &mut Vec<Erased> = tail.store.entry(ty).or_insert_with(bucket_ops.empty);
                    let cloned = (bucket_ops.clone)(*head);
                    let () = (bucket_ops.push)(v, cloned);
                    return Some(tail);
                }
                if let Some((drop_head, _)) = self.recurse.take() {
                    let () = (bucket_ops.drop)(drop_head);
                }
            }

            if self.index >= self.store.store.get(&ty)?.len() {
                return None;
            }
            let mut ablated = self.store.clone();
            let v = ablated.store.get_mut(&ty)?;
            let head = (bucket_ops.swap_remove)(v, self.index);
            #[expect(
                clippy::arithmetic_side_effects,
                reason = "hardware can't support `usize::MAX` elements in a vector"
            )]
            let () = self.index += 1;

            self.recurse = Some((head, Box::new(Self::new(ablated, requirements.clone()))));
        }
    }
}

impl Fields for Store {
    #[inline]
    #[expect(
        clippy::expect_used,
        reason = "Internal invariants: violations should fail loudly."
    )]
    fn field<T>(&mut self) -> T
    where
        T: Pbt,
    {
        self.pop().expect("INTERNAL ERROR (`pbt`): missing field")
    }
}

impl Store {
    /// Drop all unused fields of this store.
    /// If this is not called, stores must
    /// use all their stored fields before being dropped.
    #[inline]
    pub(crate) fn drop_unused(&mut self) {
        #[expect(clippy::iter_over_hash_type, reason = "order doesn't matter")]
        for (k, v) in self.store.drain() {
            let bucket_ops = bucket_ops_of(k);
            let () = (bucket_ops.drop_vec)(v);
        }
    }

    /// An empty collection of fields of arbitrary/mixed types.
    #[inline]
    #[must_use]
    pub const fn new() -> Self {
        Self { store: map() }
    }

    /// Pop and return a cached field of this type iff one exists.
    #[inline]
    pub(crate) fn pop<T>(&mut self) -> Option<T>
    where
        T: 'static,
    {
        let ty = TypeId::of::<T>();
        let hash_map::Entry::Occupied(mut entry) = self.store.entry(ty) else {
            return None;
        };
        let erased: &mut Vec<Erased> = entry.get_mut();
        // SAFETY: Invariant. Extremely dangerous.
        let typed: &mut Vec<T> =
            unsafe { ptr::from_mut(erased).cast::<Vec<T>>().as_mut_unchecked() };
        let t = typed.pop()?;
        if typed.is_empty() {
            let erased_to_drop: Vec<Erased> = entry.remove();
            // SAFETY: Invariant. Extremely dangerous.
            let typed_to_drop: Vec<T> =
                unsafe { mem::transmute::<Vec<Erased>, Vec<T>>(erased_to_drop) };
            let () = drop(typed_to_drop);
        }
        Some(t)
    }

    /// Pop and return all cached fields of this type iff they exist.
    #[inline]
    #[cfg_attr(not(test), expect(dead_code, reason = "TODO"))]
    pub(crate) fn pop_all<T>(&mut self) -> Option<Vec<T>>
    where
        T: 'static,
    {
        // SAFETY: Invariant. Extremely dangerous.
        unsafe {
            mem::transmute::<Option<Vec<Erased>>, Option<Vec<T>>>(
                self.store.remove(&TypeId::of::<T>()),
            )
        }
    }

    /// Pop and return a cached field of any type iff one exists.
    #[inline]
    pub(crate) fn pop_erased(&mut self) -> Option<(TypeId, ptr::NonNull<Erased>)> {
        let ty = *self.store.keys().next()?;
        let bucket_ops = bucket_ops_of(ty);
        let hash_map::Entry::Occupied(mut entry) = self.store.entry(ty) else {
            panic!("INTERNAL ERROR (`pbt`): disappearing store items")
        };
        let erased: &mut Vec<Erased> = entry.get_mut();
        let popped =
            (bucket_ops.pop)(erased).expect("INTERNAL ERROR (`pbt`): empty vector in a `Store`");
        if erased.is_empty() {
            let erased_to_drop: Vec<Erased> = entry.remove();
            let () = (bucket_ops.drop_vec)(erased_to_drop);
        }
        Some((ty, popped))
    }

    /// Store a field of this type.
    #[inline]
    pub fn push<T>(&mut self, t: T)
    where
        T: Pbt,
    {
        let () = register_globally::<T>();
        let () = self.push_erased(
            TypeId::of::<T>(),
            ptr::NonNull::from_mut(Box::leak(Box::new(t))).cast(),
        );
    }

    /// Store a field of some type.
    #[inline]
    pub(crate) fn push_erased(&mut self, ty: TypeId, erased_boxed: ptr::NonNull<Erased>) {
        let bucket_ops = bucket_ops_of(ty);
        let v: &mut Vec<Erased> = self.store.entry(ty).or_insert_with(bucket_ops.empty);
        let () = (bucket_ops.push)(v, erased_boxed);
    }

    /// Iterate over all possible subsets and orderings
    /// using these stored fields to create a sub-store
    /// containing a requested multiset of types.
    #[inline]
    pub(crate) fn sections(self, requirements: Multiset<TypeId>) -> impl Iterator<Item = Self> {
        Sections::new(self, requirements)
    }

    /// Visit all sub-terms of an arbitrary type within a `Store`.
    #[inline]
    pub(crate) fn visit<T>(self) -> impl Iterator<Item = T>
    where
        T: Pbt,
    {
        Visitor::<T>::new(self)
    }
}

impl Clone for Store {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            store: self
                .store
                .iter()
                .map(|(&k, v)| (k, (bucket_ops_of(k).clone_vec)(v)))
                .collect(),
        }
    }
}

impl Default for Store {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Store {
    #[inline]
    fn drop(&mut self) {
        assert!(
            self.store.is_empty(),
            "INTERNAL ERROR (`pbt`): unused fields",
        );
    }
}

impl<T> Visitor<T>
where
    T: Pbt,
{
    /// Visit all sub-terms of an arbitrary type within a `Store`.
    #[inline]
    fn new(store: Store) -> Self {
        let ty = TypeId::of::<T>();
        Self {
            bucket_ops: bucket_ops_of(ty),
            matches: vec![],
            queue: None,
            recurse: None,
            store,
            ty,
        }
    }
}

impl<T> Iterator for Visitor<T>
where
    T: Pbt,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        'restart: loop {
            if let Some(t) = self.matches.pop() {
                return Some(t);
            }

            if let Some(ref mut recurse) = self.recurse {
                if let Some(next) = recurse.next() {
                    return Some(next);
                }
                self.recurse = None;
            }

            while let Some(ref mut queue) = self.queue
                && let Some(boxed_erased) = (self.bucket_ops.pop)(queue)
            {
                let mut fields = (self.bucket_ops.deconstruct)(boxed_erased).fields;
                if !is_literal(self.ty) {
                    self.recurse = Some(Box::new(Self::new(fields)));
                    continue 'restart;
                }
                let () = fields.drop_unused();
            }

            if let Some(queue) = self.queue.take() {
                let () = (self.bucket_ops.drop_vec)(queue);
            }

            self.ty = *self.store.store.keys().next()?;
            self.bucket_ops = bucket_ops_of(self.ty);
            self.queue = self.store.store.remove(&self.ty);

            if self.ty == TypeId::of::<T>()
                && let Some(ref queue) = self.queue
            {
                // SAFETY: Invariant. Extremely dangerous.
                self.matches = unsafe {
                    mem::transmute::<Vec<Erased>, Vec<T>>((self.bucket_ops.clone_vec)(queue))
                };
            }
        }
    }
}

impl<T> Drop for Visitor<T> {
    #[inline]
    fn drop(&mut self) {
        if let Some(queue) = self.queue.take() {
            let () = (self.bucket_ops.drop_vec)(queue);
        }
        let () = self.store.drop_unused();
    }
}

#[cfg(test)]
mod tests {
    #![expect(clippy::unwrap_used, reason = "Failing tests ought to panic.")]

    use {super::*, crate::arbitrary, core::iter, pretty_assertions::assert_eq};

    // TODO: make this a real PBT when macro are ready
    #[test]
    fn lossless() {
        let mut prng = WyRand::new(42);
        for ints in arbitrary::<Vec<usize>>(&mut prng).unwrap().take(10) {
            let mut store = Store::new();
            for &int in ints.iter().rev() {
                let () = store.push(int);
            }
            let reconstructed: Vec<usize> = iter::from_fn(|| store.pop()).collect();
            assert_eq!(reconstructed, ints);
        }
    }

    #[test]
    fn sections_123_1() {
        let mut store = Store::new();
        let () = store.push(1_usize);
        let () = store.push(2_usize);
        let () = store.push(3_usize);
        assert_eq!(
            store
                .sections(iter::once((TypeId::of::<usize>(), 1)).collect())
                .map(|mut s| s.pop_all::<usize>().unwrap())
                .collect::<Vec<Vec<usize>>>(),
            vec![vec![1], vec![2], vec![3]],
        );
    }

    #[test]
    fn sections_123_2() {
        let mut store = Store::new();
        let () = store.push(1_usize);
        let () = store.push(2_usize);
        let () = store.push(3_usize);
        assert_eq!(
            store
                .sections(iter::once((TypeId::of::<usize>(), 2)).collect())
                .map(|mut s| s.pop_all::<usize>().unwrap())
                .collect::<Vec<Vec<usize>>>(),
            vec![
                vec![3, 1],
                vec![2, 1],
                vec![1, 2],
                vec![3, 2],
                vec![1, 3],
                vec![2, 3],
            ],
        );
    }

    #[test]
    fn sections_123_3() {
        let mut store = Store::new();
        let () = store.push(1_usize);
        let () = store.push(2_usize);
        let () = store.push(3_usize);
        assert_eq!(
            store
                .sections(iter::once((TypeId::of::<usize>(), 3)).collect())
                .map(|mut s| s.pop_all::<usize>().unwrap())
                .collect::<Vec<Vec<usize>>>(),
            vec![
                vec![2, 3, 1],
                vec![3, 2, 1],
                vec![3, 1, 2],
                vec![1, 3, 2],
                vec![2, 1, 3],
                vec![1, 2, 3]
            ],
        );
    }

    #[test]
    fn sections_vec_123() {
        let mut store = Store::new();
        let () = store.push(vec![1_usize]);
        let () = store.push(vec![2_usize]);
        let () = store.push(vec![3_usize]);
        assert_eq!(
            store
                .sections(iter::once((TypeId::of::<Vec<usize>>(), 2)).collect())
                .map(|mut s| s.pop_all::<Vec<usize>>().unwrap())
                .collect::<Vec<Vec<Vec<usize>>>>(),
            vec![
                vec![vec![3], vec![1]],
                vec![vec![2], vec![1]],
                vec![vec![1], vec![2]],
                vec![vec![3], vec![2]],
                vec![vec![1], vec![3]],
                vec![vec![2], vec![3]],
            ],
        );
    }

    #[test]
    fn drop_sections() {
        let mut store = Store::new();
        let () = store.push(vec![1_usize]);
        let () = store.push(vec![2_usize]);
        let () = store.push(vec![3_usize]);
        let mut sections = store.sections(iter::once((TypeId::of::<Vec<usize>>(), 2)).collect());
        let () = sections.next().unwrap().drop_unused();
        // drop
    }

    #[test]
    fn visit_vec_as_linked_list() {
        let mut store = Store::new();
        let () = store.push(vec![1, 2, 3_usize]);

        let vec_store = store.clone();
        let vecs_visited: Vec<Vec<usize>> = vec_store.visit::<Vec<usize>>().collect();
        let vecs_expected: Vec<Vec<usize>> = vec![vec![1, 2, 3], vec![1, 2], vec![1], vec![]];
        assert_eq!(vecs_visited, vecs_expected);

        let usizes_visited: Vec<usize> = store.visit::<usize>().collect();
        let usizes_expected: Vec<usize> = vec![3, 2, 1];
        assert_eq!(usizes_visited, usizes_expected);
    }

    #[test]
    fn drop_visitor() {
        let mut store = Store::new();
        let () = store.push(1_usize);
        let () = store.push(2_usize);
        let () = store.push(3_usize);
        let mut visitor = store.visit::<usize>();
        let _: usize = visitor.next().unwrap();
        // drop
    }
}
