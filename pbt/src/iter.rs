//! Iterator tooling, mainly for computing Cartesian products.

#![expect(
    clippy::zero_sized_map_values,
    reason = "Until `btree_set::Entry` is stabilized."
)]

use core::{hint::unreachable_unchecked, ptr};
use std::collections::{BTreeMap, btree_map};

/// A wrapper around an iterator that drops the
/// underlying iterator the first time it returns `None`.
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum Fuse<I: Iterator> {
    /// An iterator is currently running and has not yet produced `None`.
    Active {
        /// The iterator which will be dropped when it yields `None`.
        iter: I,
    },
    /// Either no iterator has run yet or the last one just produced `None`.
    Inactive,
}

/// Lazily run an iterator an infinite number of times,
/// returning `None` exactly once between runs.
///
/// # Initial behavior
/// The first call to `next()`
/// will return `Some(..)` if and only if
/// the underlying iterator is not empty.
pub struct AutoReload<I: Iterator, F: Fn() -> I> {
    /// Fused iterator: `None` at initialization,
    /// then `Some` while running, then `None` at the end,
    /// then lazily replenished in `next()` using `F`.
    fuse: Fuse<I>,
    /// Function to lazily compute an iterator of type `I`.
    reload: F,
}

/// Lazily compute one value at a time,
/// caching that value and cloning it endlessly
/// until the cache is cleared, at which point
/// the cycle continues until the underlying iterator returns `None`.
pub struct Cache<I: Iterator>
where
    I::Item: Clone,
{
    /// The last returned item, if any,
    /// unless it has been cleared.
    cache: Option<I::Item>,
    /// An iterator of type `I`.
    iter: I,
}

/// Compute all pairs in which the left-hand side comes from the left iterator
/// and the right-hand side comes from the right iterator.
///
/// # Order
/// The right-hand iterator iterates at the usual speed;
/// the left-hand iterator returns one value at a time,
/// repeating each until the right-hand iterator finishes an entire iteration,
/// at which point the right-hand iterator is reset and the left advances.
///
/// For example, the following uses `0..3` on both the left- and right-hand sides:
/// ```rust
/// let mut iter = pbt::iter::CartesianProduct::new(0..3_u8, || 0..3_u8);
/// assert_eq!(iter.next(), Some((0, 0)));
/// assert_eq!(iter.next(), Some((0, 1)));
/// assert_eq!(iter.next(), Some((0, 2)));
/// assert_eq!(iter.next(), Some((1, 0)));
/// assert_eq!(iter.next(), Some((1, 1)));
/// assert_eq!(iter.next(), Some((1, 2)));
/// assert_eq!(iter.next(), Some((2, 0)));
/// assert_eq!(iter.next(), Some((2, 1)));
/// assert_eq!(iter.next(), Some((2, 2)));
/// assert_eq!(iter.next(), None);
/// ```
pub struct CartesianProduct<Head: Iterator, Tail: Iterator, F: Fn() -> Tail>
where
    Head::Item: Clone,
{
    /// A caching iterator over the left-hand side.
    head: Cache<Head>,
    /// An automatically reloaded iterator over the right-hand side.
    tail: AutoReload<Tail, F>,
}

/// Lazily remove duplicates from an iterator.
///
/// Note that this will take more and more space
/// the more unique elements `I` creates over time.
pub struct RemoveDuplicates<I: Iterator>
where
    I::Item: Clone + Ord,
{
    /// An iterator of type `I` which may or may not
    /// produce the same value(s) multiple times,
    /// separated or not by additional items.
    iter: I,
    /// Set of values seen so far.
    seen: BTreeMap<I::Item, ()>, // Not a `BTreeSet` until `btree_set::Entry` is stabilized.
}

impl<I: Iterator> Fuse<I> {
    /// Preload an iterator that will be dropped as soon as it returns `None`.
    #[inline]
    pub const fn new(iter: I) -> Self {
        Self::Active { iter }
    }

    /// Reload a new iterator, assuming that no iterator is currently active.
    /// # Panics
    /// If an iterator is already active.
    /// Note that in release mode, instead of panicking,
    /// the active iterator will simply not be dropped,
    /// which could be an even worse outcome.
    #[inline]
    pub const fn reload(&mut self, iter: I) {
        #[cfg(all(test, debug_assertions))]
        #[expect(clippy::panic, reason = "Configured for tests only.")]
        let Self::Inactive = *self else {
            panic!("Called `Fuse::reload` on an active iterator!");
        };
        // SAFETY: Nothing to drop if `Self::Inactive`, checked above.
        unsafe {
            let () = ptr::write(self, Self::Active { iter });
        }
    }
}

impl<I: Iterator, F: Fn() -> I> AutoReload<I, F> {
    /// Lazily call this function to produce an iterator as necessary,
    /// returning `None` exactly once at the end of every iterator's run
    /// (but NOT at the beginning, unless of course this iterator is empty).
    #[inline]
    pub const fn new(reload: F) -> Self {
        Self {
            fuse: Fuse::Inactive,
            reload,
        }
    }
}

impl<I: Iterator> Cache<I>
where
    I::Item: Clone,
{
    /// Reload a new iterator, assuming that no iterator is currently active.
    /// # Panics
    /// If no element is cached.
    /// Note that in release mode, instead of panicking,
    /// a nonexistent value will be dropped (whatever remains on the stack),
    /// which ALMOST SURELY WILL be worse than an intentional crash.
    #[inline]
    pub fn clear(&mut self) {
        #[cfg(all(test, debug_assertions))]
        #[expect(clippy::panic, reason = "Configured for tests only.")]
        let Some(..) = self.cache else {
            panic!("Called `Cache::clear` on an empty cache!");
        };
        // SAFETY:
        // Checked above.
        #[expect(clippy::multiple_unsafe_ops_per_block, reason = "Logically connected.")]
        unsafe {
            let Some(ref mut cache) = self.cache else {
                // SAFETY:
                unreachable_unchecked()
            };
            let () = ptr::drop_in_place(cache);
            // SAFETY: Already dropped above.
            let () = ptr::write(&raw mut self.cache, None);
        }
    }

    /// Preload an iterator that will lazily compute a cached element
    /// and endlessly return that cached element until cleared,
    /// at which point the cycle repeats until
    /// the underlying iterator returns `None`.
    #[inline]
    pub const fn new(iter: I) -> Self {
        Self { iter, cache: None }
    }
}

impl<Head: Iterator, Tail: Iterator, F: Fn() -> Tail> CartesianProduct<Head, Tail, F>
where
    Head::Item: Clone,
{
    /// Preload two iterators, the first of which will cache its item
    /// until the second has fully exhausted all items,
    /// and the second of which will restart until the first runs out.
    #[inline]
    pub const fn new(head: Head, tail: F) -> Self {
        Self {
            head: Cache::new(head),
            tail: AutoReload::new(tail),
        }
    }
}

impl<I: Iterator> RemoveDuplicates<I>
where
    I::Item: Clone + Ord,
{
    /// Lazily remove duplicates from an iterator.
    ///
    /// Note that this will take more and more space
    /// the more unique elements `I` creates over time.
    #[inline]
    pub const fn new(iter: I) -> Self {
        Self {
            iter,
            seen: BTreeMap::new(),
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<I: Iterator> Iterator for Fuse<I> {
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let Self::Active { ref mut iter } = *self else {
            return None;
        };
        let Some(item) = iter.next() else {
            *self = Self::Inactive; // <-- This is the key line!
            return None;
        };
        Some(item)
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<I: Iterator, F: Fn() -> I> Iterator for AutoReload<I, F> {
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if matches!(self.fuse, Fuse::Inactive) {
            let () = self.fuse.reload((self.reload)());
        }
        self.fuse.next()
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<I: Iterator> Iterator for Cache<I>
where
    I::Item: Clone,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(ref cache) = self.cache {
            Some(cache.clone())
        } else {
            let item = self.iter.next()?; // <-- The only point of failure.
            // SAFETY: Nothing to drop if `None`, checked above.
            unsafe {
                let () = ptr::write(&raw mut self.cache, Some(item.clone()));
            }
            Some(item)
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Head: Iterator, Tail: Iterator, F: Fn() -> Tail> Iterator for CartesianProduct<Head, Tail, F>
where
    Head::Item: Clone,
{
    type Item = (Head::Item, Tail::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let head = self.head.next()?; // Clone cached or lazily generate.
            if let Some(tail) = self.tail.next() {
                return Some((head, tail));
            }
            let () = self.head.clear();
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<I: Iterator> Iterator for RemoveDuplicates<I>
where
    I::Item: Clone + Ord,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.iter.next()?;
            if let btree_map::Entry::Vacant(vacant) = self.seen.entry(item) {
                let item = vacant.key().clone();
                let () = *vacant.insert(());
                return Some(item);
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn fuse_does_not_repeat() {
        let mut iter = Fuse::new(0..3_u8);
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Called `Fuse::reload` on an active iterator!")]
    fn fuse_reload_active() {
        let () = Fuse::new(0..3_u8).reload(0..3_u8);
    }

    #[test]
    fn auto_reload_repeats() {
        let mut iter = AutoReload::new(|| 0..3_u8);
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), Some(0));
        assert_eq!(iter.next(), Some(1));
        assert_eq!(iter.next(), Some(2));
        assert_eq!(iter.next(), None);
    }

    #[test]
    #[cfg(debug_assertions)]
    #[should_panic(expected = "Called `Cache::clear` on an empty cache!")]
    fn cache_clear_none() {
        let () = Cache::new(0..3_u8).clear();
    }

    #[test]
    fn cartesian_product_does_not_repeat() {
        let mut iter = CartesianProduct::new(0..3_u8, || 0..3_u8);
        assert_eq!(iter.next(), Some((0, 0)));
        assert_eq!(iter.next(), Some((0, 1)));
        assert_eq!(iter.next(), Some((0, 2)));
        assert_eq!(iter.next(), Some((1, 0)));
        assert_eq!(iter.next(), Some((1, 1)));
        assert_eq!(iter.next(), Some((1, 2)));
        assert_eq!(iter.next(), Some((2, 0)));
        assert_eq!(iter.next(), Some((2, 1)));
        assert_eq!(iter.next(), Some((2, 2)));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None); // <-- This is the real test!
        assert_eq!(iter.next(), None);
    }

    #[test]
    #[expect(clippy::cognitive_complexity, reason = "Just a bunch of tuples.")]
    fn cartesian_triple() {
        let mut iter =
            CartesianProduct::new(0..3_u8, || CartesianProduct::new(0..3_u8, || 0..3_u8));

        assert_eq!(iter.next(), Some((0, (0, 0))));
        assert_eq!(iter.next(), Some((0, (0, 1))));
        assert_eq!(iter.next(), Some((0, (0, 2))));
        assert_eq!(iter.next(), Some((0, (1, 0))));
        assert_eq!(iter.next(), Some((0, (1, 1))));
        assert_eq!(iter.next(), Some((0, (1, 2))));
        assert_eq!(iter.next(), Some((0, (2, 0))));
        assert_eq!(iter.next(), Some((0, (2, 1))));
        assert_eq!(iter.next(), Some((0, (2, 2))));
        assert_eq!(iter.next(), Some((1, (0, 0))));
        assert_eq!(iter.next(), Some((1, (0, 1))));
        assert_eq!(iter.next(), Some((1, (0, 2))));
        assert_eq!(iter.next(), Some((1, (1, 0))));
        assert_eq!(iter.next(), Some((1, (1, 1))));
        assert_eq!(iter.next(), Some((1, (1, 2))));
        assert_eq!(iter.next(), Some((1, (2, 0))));
        assert_eq!(iter.next(), Some((1, (2, 1))));
        assert_eq!(iter.next(), Some((1, (2, 2))));
        assert_eq!(iter.next(), Some((2, (0, 0))));
        assert_eq!(iter.next(), Some((2, (0, 1))));
        assert_eq!(iter.next(), Some((2, (0, 2))));
        assert_eq!(iter.next(), Some((2, (1, 0))));
        assert_eq!(iter.next(), Some((2, (1, 1))));
        assert_eq!(iter.next(), Some((2, (1, 2))));
        assert_eq!(iter.next(), Some((2, (2, 0))));
        assert_eq!(iter.next(), Some((2, (2, 1))));
        assert_eq!(iter.next(), Some((2, (2, 2))));
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn remove_duplicates_12345() {
        let unfiltered = [1, 2, 1, 3, 2, 4, 3, 1, 4, 1, 5, 2_u8];
        let iter = RemoveDuplicates::new(unfiltered.into_iter());
        assert_eq!(iter.collect::<Vec<_>>(), vec![1, 2, 3, 4, 5]);
    }
}
