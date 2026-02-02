use {
    crate::{
        conjure::{Conjure, Seed},
        count::{Cardinality, Count},
    },
    core::{
        fmt, iter,
        ops::{Deref, DerefMut},
    },
};

/// This is a binary tree with *nothing* at each node,
/// represented as a vector (which you can think of as the left "spine" of the tree)
/// of this self-same data structure (which are children/subtrees branching off to the right).
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[expect(clippy::exhaustive_structs, reason = "intentional")]
pub struct Decomposition(pub Vec<Self>);

pub trait Decompose: Sized {
    /// Decompose this value into a
    /// binary tree with no data at each node.
    /// # Invariant
    /// This function must "round-trip" with `from_decomposition`: that is,
    /// `Self::from_decomposition(self.decompose()) == self`, but
    /// `self.decompose(Self::from_decomposition(d) == d` need not hold,
    /// e.g. if `d` has some extra complications or redundancies that cancel out.
    #[must_use]
    fn decompose(&self) -> Decomposition;

    /// Create a term of this type from a
    /// binary tree with no data at each node.
    /// If the binary tree has extraneous data, simply ignore it:
    /// use only the minimal set of data such that
    /// every term of this type can be created from (at least) one tree.
    /// # Invariant
    /// This function must return `None` if and only if
    /// this type is uninstantiable with finite memory
    /// (i.e. all empty or inductive, e.g. uninstantiable like `!` or infinite like `struct Y(Box<Self>)`).
    #[must_use]
    fn from_decomposition(d: &[Decomposition]) -> Option<Self>;
}

impl Decomposition {
    #[inline]
    pub fn shrink<T: fmt::Debug + Decompose, P: Fn(T) -> bool>(
        counterexample: &[Self],
        property: P,
    ) -> Self {
        Self::shrink_vertical::<T, _>(counterexample, &move |d| {
            let Some(tmp) = T::from_decomposition(d) else {
                return true;
            };
            print!("{tmp:?}");
            let success = property(tmp);
            println!(" {}", if success { ' ' } else { 'X' });
            success
        })
    }

    #[inline]
    pub fn shrink_vertical<T: Decompose, P: for<'d> Fn(&'d [Self]) -> bool + ?Sized>(
        counterexample: &[Self],
        property: &P,
    ) -> Self {
        let mut acc = Self::shrink_horizontal::<T, P>(counterexample, property);
        for i in 0..acc.len() {
            *unsafe { acc.get_unchecked_mut(i) } =
                Self::shrink_vertical::<T, dyn for<'d> Fn(&'d [Self]) -> bool>(
                    unsafe { acc.get_unchecked(i) },
                    &|d: &[Self]| {
                        let reinserted: Vec<Self> = acc
                            .iter()
                            .enumerate()
                            .map(|(j, orig)| {
                                if j == i {
                                    Self(d.to_vec())
                                } else {
                                    orig.clone()
                                }
                            })
                            .collect();
                        property(&reinserted)
                    },
                );
        }

        Self(acc)
    }

    // TODO: more efficient
    #[inline]
    pub fn shrink_horizontal<T: Decompose, P: Fn(&[Self]) -> bool + ?Sized>(
        counterexample: &[Self],
        property: &P,
    ) -> Vec<Self> {
        let mut acc = Self::shrink_horizontal_contiguous::<T, P>(counterexample, property).to_vec();
        let mut index = acc.len();
        while let Some(i) = index.checked_sub(1) {
            index = i;

            let ablated = {
                let mut v = acc.clone();
                let _: Decomposition = v.remove(i);
                v
            };

            if property(&ablated) {
                acc = ablated;
                index = acc.len();
            }
        }
        acc
    }

    #[inline]
    pub fn shrink_horizontal_contiguous<'c, T: Decompose, P: Fn(&[Self]) -> bool + ?Sized>(
        counterexample: &'c [Self],
        property: &P,
    ) -> &'c [Self] {
        // debug_assert!(property(counterexample));

        let mut jump = {
            let leading_zeros = counterexample.len().leading_zeros();
            let Some(shr) = leading_zeros.checked_sub(1) else {
                return &[];
            };
            (const { isize::MIN.cast_unsigned() }) >> shr
        };
        let mut lhs = 0_usize;
        let mut rhs = jump;
        while {
            jump >>= 1_u8;
            jump != 0
        } {
            debug_assert!(jump <= counterexample.len());

            // Advance `lhs` temporarily:
            {
                let tmp_lhs = unsafe { lhs.unchecked_add(jump) };
                if tmp_lhs <= counterexample.len()
                    && property(unsafe {
                        counterexample.get_unchecked(tmp_lhs..rhs.min(counterexample.len()))
                    })
                {
                    lhs = tmp_lhs;
                }
            }

            // Advance `rhs` temporarily:
            {
                let tmp_rhs = unsafe { rhs.unchecked_sub(jump) };
                if tmp_rhs >= counterexample.len()
                    || property(unsafe {
                        counterexample.get_unchecked(lhs..tmp_rhs.min(counterexample.len()))
                    })
                {
                    rhs = tmp_rhs;

                    // TODO: retry advancing `lhs` again?
                }
            }
        }
        unsafe { counterexample.get_unchecked(lhs..rhs.min(counterexample.len())) }
    }
}

impl fmt::Debug for Decomposition {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl Count for Decomposition {
    const CARDINALITY: Cardinality = Cardinality::Infinite;
}

impl Conjure for Decomposition {
    #[inline]
    fn conjure(seed: Seed, size: usize) -> Option<Self> {
        Vec::conjure(seed, size).map(Self)
    }

    #[inline]
    fn corners() -> impl Iterator<Item = Self> {
        // Can't use `Vec::corners().map(Self)`, since that would recurse infinitely.
        iter::once(Self(vec![]))
    }

    #[inline]
    fn leaf(seed: Seed) -> Option<Self> {
        Vec::leaf(seed).map(Self)
    }
}

/*
impl ConjureAsync for Decomposition {
    #[inline]
    async fn conjure_async(seed: Seed, size: usize) -> Option<Self> {
        Box::pin(Vec::conjure_async(seed, size)).await.map(Self)
    }
}
*/

impl Decompose for Decomposition {
    #[inline]
    fn decompose(&self) -> Decomposition {
        self.clone()
    }

    #[inline]
    fn from_decomposition(d: &[Decomposition]) -> Option<Self> {
        Some(Self(d.to_vec()))
    }
}

impl AsRef<[Self]> for Decomposition {
    #[inline]
    fn as_ref(&self) -> &[Self] {
        &self.0
    }
}

impl Deref for Decomposition {
    type Target = [Self];

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Decomposition {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Check that `T::from_decomposition(&t.decompose()) == Some(t)`.
/// # Panics
/// If that's not the case.
#[inline]
#[expect(clippy::panic, reason = "failing tests ought to panic")]
fn check_roundtrip_once<T: Conjure + fmt::Debug + Decompose + Eq>(t: &T) {
    let decomposed = t.decompose();
    let Some(recomposed) = T::from_decomposition(&decomposed) else {
        panic!("{t:?} --> {decomposed:?} --> None =/= Some(..)");
    };
    pretty_assertions::assert_eq!(
        *t,
        recomposed,
        "{t:?} --> {decomposed:?} --> {recomposed:?} =/= {t:?}"
    );
}

/// Check that `T::from_decomposition(&t.decompose()) == Some(t)`
/// for a wide range of possible values of `t`.
/// # Panics
/// If a counterexample showed up.
#[inline]
#[expect(clippy::unwrap_used, reason = "failing tests ought to panic")]
pub fn check_roundtrip<T: Conjure + fmt::Debug + Decompose + Eq>() {
    const N_TRIALS: usize = 100;

    let mut seed = Seed::new();
    for corner in T::corners() {
        let () = check_roundtrip_once(&corner);
    }
    for size in 0..N_TRIALS {
        let t = T::conjure(seed.split(), size).unwrap();
        let () = check_roundtrip_once(&t);
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn decomposition_roundtrip() {
        let () = check_roundtrip::<Decomposition>();
    }

    #[test]
    fn shrink_list_contains_42() {
        type T = Vec<u8>;
        let property = |list: T| list.contains(&42);
        let counterexample: T = vec![1, 2, 3, 42, 101, 102, 103, 42, 201, 202, 203];
        let decomposed = counterexample.decompose();
        let shrunk = Decomposition::shrink(&decomposed, property);
        let recomposed = T::from_decomposition(&shrunk).unwrap();
        let () = pretty_assertions::assert_eq!(vec![42], recomposed);
    }

    #[test]
    fn shrink_list_contains_gt_42() {
        type T = Vec<u8>;
        let property = |list: T| list.iter().any(|&u| u >= 42);
        let counterexample: T = vec![1, 2, 3, 101, 102, 103, 201, 202, 203];
        let decomposed = counterexample.decompose();
        let shrunk = Decomposition::shrink(&decomposed, property);
        let recomposed = T::from_decomposition(&shrunk).unwrap();
        let () = pretty_assertions::assert_eq!(vec![42], recomposed);
    }
}
