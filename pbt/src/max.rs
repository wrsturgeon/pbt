use core::{cmp, convert::Infallible, num::TryFromIntError};

#[expect(
    clippy::exhaustive_enums,
    reason = "yes, *cue Peggy Lee* that is all there is"
)]
#[expect(
    clippy::arbitrary_source_item_ordering,
    reason = "In `PartialOrd` ordering, not alphabetical."
)]
#[derive(Debug)]
pub enum Max<Finite: PartialOrd> {
    Uninstantiable,
    Finite(Finite),
    Infinite,
}

#[expect(
    clippy::exhaustive_enums,
    reason = "yes, *cue Peggy Lee* that is all there is"
)]
#[derive(Clone, Copy, Debug)]
pub enum MaybeDecidable<T> {
    Decidable(T),
    AtMost(T),
}

#[expect(
    clippy::exhaustive_enums,
    reason = "yes, *cue Peggy Lee* that is all there is"
)]
#[derive(Clone, Copy, Debug)]
pub enum MaybeOverflow<T> {
    Contained(T),
    Overflow,
}

impl<Finite: PartialOrd> Max<Finite> {
    #[inline]
    pub const fn is_instantiable(&self) -> bool {
        !matches!(*self, Self::Uninstantiable)
    }

    #[inline]
    pub const fn is_trivial(&self) -> bool {
        matches!(*self, Self::Uninstantiable | Self::Finite(_))
    }

    #[inline]
    pub const fn unwrap_finite_ref(&self) -> &Finite {
        match *self {
            Self::Finite(ref finite) => finite,
            Self::Uninstantiable | Self::Infinite => panic!(),
        }
    }
}

impl Max<MaybeOverflow<usize>> {
    #[inline]
    pub const fn product(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (Self::Uninstantiable, _) | (_, Self::Uninstantiable) => Self::Uninstantiable,
            (Self::Infinite, _) | (_, Self::Infinite) => Self::Infinite,
            (Self::Finite(lhs), &Self::Finite(rhs)) => Self::Finite(lhs.plus_self(rhs)),
        }
    }
}

impl MaybeDecidable<Max<MaybeOverflow<usize>>> {
    #[inline]
    pub const fn product(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (Self::Decidable(lhs), Self::Decidable(rhs)) => Self::Decidable(lhs.product(rhs)),
            _ => Self::AtMost(self.at_most().product(rhs.at_most())),
        }
    }
}

impl Max<f32> {
    #[inline]
    pub const fn product(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (Self::Uninstantiable, _) | (_, Self::Uninstantiable) => Self::Uninstantiable,
            (Self::Infinite, _) | (_, Self::Infinite) => Self::Infinite,
            (&Self::Finite(lhs), &Self::Finite(rhs)) => Self::Finite(lhs + rhs),
        }
    }
}

impl MaybeDecidable<Max<f32>> {
    #[inline]
    pub const fn product(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (Self::Decidable(lhs), Self::Decidable(rhs)) => Self::Decidable(lhs.product(rhs)),
            _ => Self::AtMost(self.at_most().product(rhs.at_most())),
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<Finite: PartialOrd> Eq for Max<Finite> {}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<Finite: PartialOrd> PartialEq for Max<Finite> {
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
impl<Finite: PartialOrd> PartialOrd for Max<Finite> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match *self {
            Self::Uninstantiable => match *other {
                Self::Uninstantiable => Some(cmp::Ordering::Equal),
                Self::Finite(_) | Self::Infinite => Some(cmp::Ordering::Less),
            },
            Self::Finite(ref lhs) => match *other {
                Self::Uninstantiable => Some(cmp::Ordering::Greater),
                Self::Finite(ref rhs) => lhs.partial_cmp(rhs),
                Self::Infinite => Some(cmp::Ordering::Less),
            },
            Self::Infinite => match *other {
                Self::Infinite => Some(cmp::Ordering::Equal),
                Self::Uninstantiable | Self::Finite(_) => Some(cmp::Ordering::Greater),
            },
        }
    }
}

impl<T> MaybeDecidable<T> {
    #[inline]
    pub const fn unwrap_ref(&self) -> &T {
        match *self {
            Self::Decidable(ref t) => t,
            Self::AtMost(_) => panic!(),
        }
    }

    #[inline]
    pub const fn at_most(&self) -> &T {
        match *self {
            Self::Decidable(ref t) => t,
            Self::AtMost(ref t) => t,
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<T: PartialEq> PartialEq for MaybeDecidable<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self::Decidable(ref lhs) = *self else {
            return false;
        };
        let Self::Decidable(ref rhs) = *other else {
            return false;
        };
        lhs.eq(rhs)
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<T: PartialOrd> PartialOrd for MaybeDecidable<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        let Self::Decidable(ref lhs) = *self else {
            return None;
        };
        let Self::Decidable(ref rhs) = *other else {
            return None;
        };
        lhs.partial_cmp(rhs)
    }
}

impl<T> MaybeOverflow<T> {
    #[inline]
    pub const fn unwrap_ref(&self) -> &T {
        match *self {
            Self::Contained(ref t) => t,
            Self::Overflow => panic!(),
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<T: PartialEq> PartialEq for MaybeOverflow<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        let Self::Contained(ref lhs) = *self else {
            return false;
        };
        let Self::Contained(ref rhs) = *other else {
            return false;
        };
        lhs.eq(rhs)
    }
}

#[expect(clippy::missing_trait_methods, reason = "intentional")]
impl<T: PartialOrd> PartialOrd for MaybeOverflow<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match (self, other) {
            (&Self::Contained(ref lhs), &Self::Contained(ref rhs)) => lhs.partial_cmp(rhs),
            (&Self::Contained(_), &Self::Overflow) => Some(cmp::Ordering::Less),
            (&Self::Overflow, &Self::Contained(_)) => Some(cmp::Ordering::Greater),
            (&Self::Overflow, &Self::Overflow) => None,
        }
    }
}

impl<T> From<Result<T, TryFromIntError>> for MaybeOverflow<T> {
    #[inline]
    fn from(value: Result<T, TryFromIntError>) -> Self {
        value.map_or_else(|_| Self::Overflow, Self::Contained)
    }
}

impl<T> From<Result<T, Infallible>> for MaybeOverflow<T> {
    #[inline]
    fn from(Ok(ok): Result<T, Infallible>) -> Self {
        Self::Contained(ok)
    }
}
