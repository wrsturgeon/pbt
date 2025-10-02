//! `Decimate` implementation for `Vec<_>`.

use {
    crate::{iter::Cache, traits::decimate::Decimate},
    core::{fmt, ptr},
};

/// Decimate a slice of values,
/// returning each decimation as a `Vec<_>`.
#[derive(Clone, Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum Iter<T: Clone + fmt::Debug + Decimate>
where
    T::Decimate: Clone + fmt::Debug,
{
    /// Non-empty slice, split into its head and tail.
    Cons {
        /// The original value of the first element,
        /// for use when decimating to a new weight.
        original: T,
        /// The expected total weight of the decimated vector.
        /// This is `Some(..)` until the head is fully exhausted,
        /// at which point the head is skipped and
        /// the tail is reset to this overall weight.
        weight_alone: Option<usize>,
        /// The weight of decimations to the first element.
        /// This is initialized to the maximum possible weight
        /// (after accounting for the length of the tail)
        /// then decremented down to zero, then set to `None`
        /// to indicate that this iterator is finished.
        head_weight: Option<usize>,
        /// Caching iterator over decimations to the first element,
        /// each of which is of weight `head_weight` (if any).
        head: Option<Cache<T::Decimate>>,
        /// Iterator over the rest of the slice (same logic as here).
        tail: Box<Self>,
    },
    /// Empty slice.
    Nil {
        /// Remaining weight that has not been decimated by preceding elements.
        /// Note that `Iterator::next()` will produce `Some(_)`
        /// if and only if this field is `Some(0)`
        /// (meaning that the total weight is exactly right),
        /// and upon doing so, this will be set to `None`.
        remaining_weight: Option<usize>,
    },
}

impl<T: Clone + fmt::Debug + Decimate> Iter<T>
where
    T::Decimate: Clone + fmt::Debug,
{
    /// Increase the decimation weight of the first element,
    /// clearing the iterator if any (which would have produced an outdated weight).
    #[inline]
    #[expect(
        clippy::arithmetic_side_effects,
        reason = "If a `usize` overflows, crashing is probably the best option."
    )]
    pub fn increment_weight(&mut self) {
        println!();
        println!("Incrementing weight: {self:#?}");
        match *self {
            Self::Nil {
                ref mut remaining_weight,
            } => *remaining_weight = Some(remaining_weight.map_or(1, |weight| weight + 1)),
            Self::Cons {
                ref mut weight_alone,
                ref mut head_weight,
                ref mut head,
                ..
            } => {
                *weight_alone = Some(weight_alone.map_or(0, |weight| weight + 1));
                *head_weight = Some(head_weight.map_or(0, |weight| weight + 1));
                *head = None;
            }
        }
    }

    /// The number of elements in the original slice.
    #[inline]
    pub const fn len(&self) -> usize {
        let mut acc: usize = 0;
        let mut scrutinee: &Self = self;
        while let Self::Cons { ref tail, .. } = *scrutinee {
            // SAFETY: `self` fit in memory, so its length will fit in a `usize`.
            acc = unsafe { acc.unchecked_add(1) };
            scrutinee = tail;
        }
        acc
    }

    /// Prepare to decimate this slice.
    #[inline]
    pub fn new(slice: &[T], weight: usize) -> Self {
        match *slice {
            [] => Self::Nil {
                remaining_weight: Some(weight),
            },
            [ref head, ref tail @ ..] => {
                let singleton_weight = weight.checked_sub(1);
                Self::Cons {
                    original: head.clone(),
                    weight_alone: singleton_weight,
                    head_weight: singleton_weight,
                    head: None,
                    tail: Box::new(Self::new_with_weight_zero(tail, weight)),
                }
            }
        }
    }

    /// Prepare to decimate this slice, assigning each element a weight of `Some(0)`.
    #[inline]
    fn new_with_weight_zero(slice: &[T], weight: usize) -> Self {
        match *slice {
            [] => Self::Nil {
                remaining_weight: Some(0),
            },
            [ref head, ref tail @ ..] => Self::Cons {
                original: head.clone(),
                weight_alone: None,
                head_weight: None,
                head: None,
                tail: Box::new(Self::new_with_weight_zero(tail, weight)),
            },
        }
    }

    /// Build a vector incrementally instead of concatenating `O(n)` times
    /// (which would have brought the total runtime to `O(n^2)`).
    #[inline]
    pub fn next_acc(&mut self, acc: &mut Vec<T>) -> Option<()> {
        match *self {
            Self::Nil {
                ref mut remaining_weight,
            } => {
                println!();
                println!("Entered `next_acc` and matched `Nil`:");
                println!("{acc:?}");
                println!("Nil {{ remaining_weight: {remaining_weight:?} }}");
                let opt = (remaining_weight.take()? == 0).then_some(());
                if opt.is_some() {
                    println!();
                    println!("======> {acc:?}");
                }
                opt
            }
            Self::Cons {
                ref original,
                weight_alone,
                ref mut head_weight,
                ref mut head,
                ref mut tail,
            } => 'head_weights: loop {
                println!();
                println!("Top of `'head_weights: loop ...`");
                println!("{acc:?}");
                println!(
                    "{:#?}",
                    Self::Cons {
                        original: original.clone(),
                        weight_alone,
                        head_weight: *head_weight,
                        head: head.clone(),
                        tail: tail.clone(),
                    },
                );
                let Some(current_head_weight) = *head_weight else {
                    println!("Skipping the head...");
                    return tail.next_acc(acc);
                };
                loop {
                    let current_head_iter = head.get_or_insert_with(move || {
                        Cache::new(original.decimate(current_head_weight))
                    });
                    let Some(current_head) = current_head_iter.next() else {
                        *head_weight = current_head_weight.checked_sub(1);
                        if head_weight.is_none() {
                            let () = tail.reset(weight_alone);
                        }
                        // SAFETY: We know that `head` is `Some(..)`, so we can
                        // drop the value without checking if it's `None`,
                        // then overwrite it without dropping it a second time.
                        #[expect(
                            clippy::multiple_unsafe_ops_per_block,
                            reason = "Logically connected."
                        )]
                        unsafe {
                            let () = ptr::drop_in_place(current_head_iter);
                            let () = ptr::write(head, None);
                        }
                        let () = tail.increment_weight();
                        continue 'head_weights;
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

    /// Reset this iterator to produce decimated values of a given weight
    /// while maintaining the existing original elements (without copying them into a slice).
    #[inline]
    pub fn reset(&mut self, maybe_weight: Option<usize>) {
        println!();
        println!("Resetting (maybe_weight is `{maybe_weight:?}`): {self:#?}");
        match *self {
            Self::Nil {
                ref mut remaining_weight,
            } => *remaining_weight = maybe_weight,
            Self::Cons {
                ref mut head_weight,
                ref mut head,
                ref mut tail,
                ..
            } => {
                *head_weight = maybe_weight.and_then(|weight| weight.checked_sub(1));
                *head = None;
                let () = tail.reset_with_weight_zero();
            }
        }
    }

    /// Reset this iterator to produce decimated values of a given weight
    /// while maintaining the existing original elements (without copying them into a slice),
    /// assigning each element a weight of `Some(0)`..
    #[inline]
    fn reset_with_weight_zero(&mut self) {
        match *self {
            Self::Nil {
                ref mut remaining_weight,
            } => *remaining_weight = Some(0),
            Self::Cons {
                ref mut head_weight,
                ref mut head,
                ref mut tail,
                ..
            } => {
                *head_weight = None;
                *head = None;
                let () = tail.reset_with_weight_zero();
            }
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<T: Clone + fmt::Debug + Decimate> Iterator for Iter<T>
where
    T::Decimate: Clone + fmt::Debug,
{
    type Item = Vec<T>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let mut acc = vec![];
        let () = self.next_acc(&mut acc)?;
        Some(acc)
    }
}

impl<T: Clone + fmt::Debug + Decimate> Decimate for Vec<T>
where
    T::Decimate: Clone + fmt::Debug,
{
    type Decimate = Iter<T>;
    #[inline]
    fn decimate(&self, weight: usize) -> Self::Decimate {
        Iter::new(self, weight)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn decimate_empty_vec() {
        let orig: Vec<()> = vec![];
        {
            println!();
            println!("%%%%%%%% 0");
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        println!();
        println!("%%%%%%%% 1");
        assert_eq!(orig.decimate(1).next(), None);
        println!();
    }

    #[test]
    fn decimate_singleton_vec() {
        let orig = vec![()];
        {
            println!();
            println!("%%%%%%%% 0");
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 1");
            let mut iter = orig.decimate(1);
            assert_eq!(iter.next(), Some(vec![()]));
            assert_eq!(iter.next(), None);
        }
        println!();
        println!("%%%%%%%% 2");
        assert_eq!(orig.decimate(2).next(), None);
        println!();
    }

    #[test]
    fn decimate_vec_false_true() {
        let orig = vec![false, true];
        {
            println!();
            println!("%%%%%%%% 0");
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 1");
            let mut iter = orig.decimate(1);
            assert_eq!(iter.next(), Some(vec![false]));
            assert_eq!(iter.next(), Some(vec![true]));
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 2");
            let mut iter = orig.decimate(2);
            assert_eq!(iter.next(), Some(vec![false, true]));
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 3");
            let mut iter = orig.decimate(3);
            assert_eq!(iter.next(), None);
        }
        println!();
    }

    #[test]
    #[expect(clippy::cognitive_complexity, reason = "Just a bunch of vectors.")]
    fn decimate_vec_of_vec() {
        let orig = vec![vec![], vec![()], vec![(), ()]];
        /*
        {
            println!();
            println!("%%%%%%%% 0");
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 1");
            let mut iter = orig.decimate(1);
            assert_eq!(iter.next(), Some(vec![vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![]])); // TODO: `RemoveDuplicates`?
            assert_eq!(iter.next(), Some(vec![vec![]])); // ditto ^^
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 2");
            let mut iter = orig.decimate(2);
            assert_eq!(iter.next(), Some(vec![vec![], vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![()]]));
            assert_eq!(iter.next(), Some(vec![vec![()]]));
            assert_eq!(iter.next(), None);
        }
        */
        {
            println!();
            println!("%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%% 3");
            let mut iter = orig.decimate(3);
            assert_eq!(iter.next(), Some(vec![vec![], vec![()]]));
            println!();
            println!("%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%");
            assert_eq!(iter.next(), Some(vec![vec![], vec![()]]));
            println!();
            println!("%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%");
            assert_eq!(iter.next(), Some(vec![vec![], vec![], vec![]]));
            println!();
            println!("%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%");
            assert_eq!(iter.next(), Some(vec![vec![()], vec![]]));
            println!();
            println!("%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%");
            assert_eq!(iter.next(), Some(vec![vec![(), ()]]));
            println!();
            println!("%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%%");
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 4");
            let mut iter = orig.decimate(4);
            assert_eq!(iter.next(), Some(vec![vec![(), (), ()]]));
            assert_eq!(iter.next(), Some(vec![vec![(), ()], vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![()], vec![()]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![(), ()]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![()], vec![]]));
            assert_eq!(iter.next(), Some(vec![vec![], vec![], vec![()]]));
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 5");
            let mut iter = orig.decimate(5);
            // TODO
            assert_eq!(iter.next(), None);
        }
        {
            println!();
            println!("%%%%%%%% 6");
            let mut iter = orig.decimate(6);
            // TODO
            assert_eq!(iter.next(), None);
        }
        println!();
        println!("%%%%%%%% 7");
        assert_eq!(orig.decimate(7).next(), None);
    }

    // TODO: re-enable
    /*
    #[test]
    fn decimate_vec_1234() {
        let orig = vec![1, 2, 3, 4_u8];
        {
            let mut iter = orig.decimate(0);
            assert_eq!(iter.next(), Some(vec![]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(1);
            assert_eq!(iter.next(), Some(vec![1]));
            assert_eq!(iter.next(), Some(vec![2]));
            assert_eq!(iter.next(), Some(vec![3]));
            assert_eq!(iter.next(), Some(vec![4]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(2);
            assert_eq!(iter.next(), Some(vec![1, 2]));
            assert_eq!(iter.next(), Some(vec![1, 3]));
            assert_eq!(iter.next(), Some(vec![1, 4]));
            assert_eq!(iter.next(), Some(vec![2, 3]));
            assert_eq!(iter.next(), Some(vec![2, 4]));
            assert_eq!(iter.next(), Some(vec![3, 4]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(3);
            assert_eq!(iter.next(), Some(vec![1, 2, 3]));
            assert_eq!(iter.next(), Some(vec![1, 2, 4]));
            assert_eq!(iter.next(), Some(vec![1, 3, 4]));
            assert_eq!(iter.next(), Some(vec![2, 3, 4]));
            assert_eq!(iter.next(), None);
        }
        {
            let mut iter = orig.decimate(4);
            assert_eq!(iter.next(), Some(vec![1, 2, 3, 4]));
            assert_eq!(iter.next(), None);
        }
    }
    */
}
