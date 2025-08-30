use {crate::error, core::cmp};

#[expect(
    clippy::exhaustive_enums,
    reason = "yes, *cue Peggy Lee* that is all there is"
)]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "In `PartialOrd` ordering, not alphabetical."
)]
pub enum Max<Finite: Ord> {
    Uninstantiable,
    Finite(Finite),
    Infinite,
}

impl<Finite: Ord> Max<Finite> {
    #[inline]
    pub const fn is_instantiable(&self) -> bool {
        !matches!(*self, Self::Uninstantiable)
    }

    #[inline]
    pub const fn is_trivial(&self) -> bool {
        matches!(*self, Self::Uninstantiable | Self::Finite(_))
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<Finite: Ord> Eq for Max<Finite> {}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<Finite: Ord> PartialEq for Max<Finite> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match *self {
            Self::Uninstantiable => matches!(*other, Self::Uninstantiable),
            Self::Finite(ref lhs) => {
                if let Self::Finite(ref rhs) = *other {
                    lhs.eq(rhs)
                } else {
                    false
                }
            }
            Self::Infinite => matches!(*other, Self::Infinite),
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<Finite: Ord> Ord for Max<Finite> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match *self {
            Self::Uninstantiable => match *other {
                Self::Uninstantiable => cmp::Ordering::Equal,
                Self::Finite(_) | Self::Infinite => cmp::Ordering::Less,
            },
            Self::Finite(ref lhs) => match *other {
                Self::Uninstantiable => cmp::Ordering::Greater,
                Self::Finite(ref rhs) => lhs.cmp(rhs),
                Self::Infinite => cmp::Ordering::Less,
            },
            Self::Infinite => match *other {
                Self::Infinite => cmp::Ordering::Equal,
                Self::Uninstantiable | Self::Finite(_) => cmp::Ordering::Greater,
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<Finite: Ord> PartialOrd for Max<Finite> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(<Self as Ord>::cmp(self, other))
    }
}

pub trait AstSize {
    const MAX_AST_SIZE: Result<Max<usize>, error::Undecidable>;

    fn ast_size(&self) -> usize;
}
