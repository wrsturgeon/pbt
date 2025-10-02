//! `Refine` implementation for `Vec<_>`.

use {
    crate::{iter::Cache, traits::refine::Refine},
    core::ptr,
};

/// Refine a slice of values,
/// returning each refinement as a `Vec<_>`.
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum Iter<T: Clone + Refine> {
    /// Non-empty slice, split into its head and tail.
    Cons {
        /// The size of refinements to the first element.
        /// This is initialized to the maximum possible size
        /// (after accounting for the length of the tail)
        /// then decremented down to zero, then set to `None`
        /// to indicate that this iterator is finished.
        head_size: Option<usize>,
        /// Caching iterator over refinements to the first element,
        /// each of which is of size `head_size` (if any).
        head: Option<Cache<T::Refine>>,
        /// Iterator over the rest of the slice (same logic as here).
        tail: Box<Self>,
        /// The original value of the first element,
        /// for use when refining to a new size.
        original: T,
    },
    /// Empty slice.
    Nil {
        /// Remaining size that has not been refined by preceding elements.
        /// Note that `Iterator::next()` will produce `Some(_)`
        /// if and only if this field is `Some(0)`
        /// (meaning that the total size is exactly right),
        /// and upon doing so, this will be set to `None`.
        remaining_size: Option<usize>,
    },
}

impl<T: Clone + Refine> Iter<T> {
    /// Increase the refinement size of the first element,
    /// clearing the iterator if any (which would have produced an outdated size).
    #[inline]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "If a `usize` overflows, crashing is probably the best option."
    )]
    pub fn increment_size(&mut self) {
        match *self {
            Self::Nil {
                ref mut remaining_size,
            } => *remaining_size = Some(remaining_size.map_or(1, |size| size + 1)),
            Self::Cons {
                ref mut head_size,
                ref mut head,
                ..
            } => {
                *head_size = Some(head_size.map_or(0, |size| size + 1));
                *head = None;
            }
        }
    }

    /// Prepare to refine this slice.
    #[inline]
    pub fn new(slice: &[T], size: usize) -> Self {
        match *slice {
            [] => Self::Nil {
                remaining_size: Some(size),
            },
            [ref head, ref tail @ ..] => Self::Cons {
                head_size: size.checked_sub(slice.len()),
                head: None,
                tail: Box::new(Self::new_with_size_zero(tail)),
                original: head.clone(),
            },
        }
    }

    /// Prepare to refine this slice, assigning each element a size of `Some(0)`.
    #[inline]
    fn new_with_size_zero(slice: &[T]) -> Self {
        match *slice {
            [] => Self::Nil {
                remaining_size: Some(0),
            },
            [ref head, ref tail @ ..] => Self::Cons {
                head_size: Some(0),
                head: None,
                tail: Box::new(Self::new_with_size_zero(tail)),
                original: head.clone(),
            },
        }
    }

    /// Build a vector incrementally instead of concatenating `O(n)` times
    /// (which would have brought the total runtime to `O(n^2)`).
    #[inline]
    pub fn next_acc(&mut self, acc: &mut Vec<T>) -> Option<()> {
        match *self {
            Self::Nil {
                ref mut remaining_size,
            } => (remaining_size.take()? == 0).then_some(()),
            Self::Cons {
                ref mut head_size,
                ref mut head,
                ref mut tail,
                ref original,
            } => 'head_sizes: loop {
                let current_head_size = (*head_size)?;
                loop {
                    let current_head_iter = head
                        .get_or_insert_with(move || Cache::new(original.refine(current_head_size)));
                    let Some(current_head) = current_head_iter.next() else {
                        *head_size = current_head_size.checked_sub(1);
                        // SAFETY: We know that `head` is `Some(..)`, so we can
                        // drop the value without checking if it's `None,
                        // then overwrite it without dropping it a second time.
                        #[expect(
                            clippy::multiple_unsafe_ops_per_block,
                            reason = "Logically connected."
                        )]
                        unsafe {
                            let () = ptr::drop_in_place(current_head_iter);
                            let () = ptr::write(head, None);
                        }
                        let () = tail.increment_size();
                        continue 'head_sizes;
                    };
                    let () = acc.push(current_head);
                    if matches!(tail.next_acc(acc), Some(())) {
                        return Some(());
                    }
                    // SAFETY: We just pushed, and `next_acc` never pops more than it pushes.
                    let () = drop::<T>(unsafe { acc.pop().unwrap_unchecked() });
                    let () = current_head_iter.clear();
                    // TODO: Maybe make the above a bit tighter by jumping up in the loop
                    // rather than starting over entirely?
                }
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<T: Clone + Refine> Iterator for Iter<T> {
    type Item = Vec<T>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut acc = vec![];
        let () = self.next_acc(&mut acc)?;
        Some(acc)
    }
}

impl<T: Clone + Refine> Refine for Vec<T> {
    type Refine = Iter<T>;
    #[inline]
    fn refine(&self, size: usize) -> Self::Refine {
        Iter::new(self, size)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn refine_vec_false_true() {
        let orig = vec![false, true];
        {
            let mut iter = orig.refine(0);
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(1);
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(2);
            assert_eq!(iter.next(), Some(vec![false, false]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(3);
            assert_eq!(iter.next(), Some(vec![false, true]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(4);
            assert_eq!(iter.next(), None);
        }
    }

    #[test]
    fn refine_vec_of_vec() {
        let orig = vec![vec![], vec![()], vec![(), ()]];
        assert_eq!(orig.refine(0).next(), None);
        assert_eq!(orig.refine(1).next(), None);
        assert_eq!(orig.refine(2).next(), None);
        assert_eq!(orig.refine(3).next(), None);
        assert_eq!(orig.refine(4).next(), None);
        assert_eq!(orig.refine(5).next(), None);
        {
            let mut iter = orig.refine(6);
            assert_eq!(iter.next(), Some(vec![vec![], vec![()], vec![(), ()]]));
            assert_eq!(iter.next(), None);
        }
        assert_eq!(orig.refine(7).next(), None);
    }

    /*
    #[test]
    #[expect(
        clippy::cognitive_complexity,
        clippy::too_many_lines,
        reason = "Just a long iterator."
    )]
    fn refine_vec_1234() {
        let orig = vec![1, 2, 3, 4_u8];
        assert_eq!(orig.refine(0).next(), None);
        assert_eq!(orig.refine(1).next(), None);
        assert_eq!(orig.refine(2).next(), None);
        assert_eq!(orig.refine(3).next(), None);
        {
            let mut iter = orig.refine(4);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(5);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 1]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 0]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(6);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 2]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 1]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 1]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 0]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(7);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 3]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 2]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 1]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 2]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 1]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 1]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 0]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 2]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 1]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 1, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 1, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![3, 0, 0, 0]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(8);
            assert_eq!(iter.next(), Some(vec![0, 0, 0, 4]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 3]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 2]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 3]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 2]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 1]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 2]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 1]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 3]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 1]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 2]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 1]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 1]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 0]));
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 2]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(9);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 5]));
            assert_eq!(iter.next(), Some(vec![0, 0, 1, 4]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 3]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 5, 0]));
            assert_eq!(iter.next(), Some(vec![0, 1, 0, 4]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 3]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 2]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 3]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 2]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 1]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 1]));
            // ...
            assert_eq!(iter.next(), Some(vec![1, 0, 0, 4]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 2]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 3]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 1]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 2]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 1]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 1]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 3]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(10);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 1, 5]));
            assert_eq!(iter.next(), Some(vec![0, 0, 2, 4]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 0, 5]));
            assert_eq!(iter.next(), Some(vec![0, 1, 1, 4]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 3]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 5, 0]));
            assert_eq!(iter.next(), Some(vec![0, 2, 0, 4]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 3]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 2]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 3]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 0, 0, 5]));
            assert_eq!(iter.next(), Some(vec![1, 0, 1, 4]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 5, 0]));
            assert_eq!(iter.next(), Some(vec![1, 1, 0, 4]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 2]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 1]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 2]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 4]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(11);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 2, 5]));
            assert_eq!(iter.next(), Some(vec![0, 0, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 0, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 1, 5]));
            assert_eq!(iter.next(), Some(vec![0, 1, 2, 4]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 0, 5]));
            assert_eq!(iter.next(), Some(vec![0, 2, 1, 4]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 3]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 5, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 4]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 0, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 1, 5]));
            assert_eq!(iter.next(), Some(vec![1, 0, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 0, 5]));
            assert_eq!(iter.next(), Some(vec![1, 1, 1, 4]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 5, 0]));
            assert_eq!(iter.next(), Some(vec![1, 2, 0, 4]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 2]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 4, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 3]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 5]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(12);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 8]));
            // ...
            // assert_eq!(iter.next(), Some(vec![0, 1, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 2, 5]));
            assert_eq!(iter.next(), Some(vec![0, 1, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 1, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 1, 5]));
            assert_eq!(iter.next(), Some(vec![0, 2, 2, 4]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 5]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 0, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 2, 5]));
            assert_eq!(iter.next(), Some(vec![1, 0, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 0, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 1, 5]));
            assert_eq!(iter.next(), Some(vec![1, 1, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 0, 5]));
            assert_eq!(iter.next(), Some(vec![1, 2, 1, 4]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 4, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 5, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 4]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 6]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(13);
            // assert_eq!(iter.next(), Some(vec![0, 0, 0, 9]));
            // ...
            // assert_eq!(iter.next(), Some(vec![0, 2, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 2, 5]));
            assert_eq!(iter.next(), Some(vec![0, 2, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![0, 2, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![0, 3, 0, 6]));
            // ...
            // assert_eq!(iter.next(), Some(vec![1, 1, 0, 7]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 1, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 2, 5]));
            assert_eq!(iter.next(), Some(vec![1, 1, 3, 4]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 4, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 5, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 6, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 1, 7, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 0, 6]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 1, 5]));
            assert_eq!(iter.next(), Some(vec![1, 2, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 3]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 4, 2]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 5, 1]));
            // assert_eq!(iter.next(), Some(vec![1, 2, 6, 0]));
            // assert_eq!(iter.next(), Some(vec![1, 3, 0, 5]));
            // ...
            // assert_eq!(iter.next(), Some(vec![2, 0, 0, 7]));
            // ...
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.refine(14);
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 4]));
            assert_eq!(iter.next(), None);
        }
    }
    */
}
