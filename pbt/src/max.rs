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
    AtMost(T),
    Decidable(T),
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
    /// Check if this type is instantiable.
    /// Not a paradox since this is wrapped in `MaybeDecidable`,
    /// so complex-enough types will never be able to call this in the first place.
    #[inline]
    pub const fn is_instantiable(&self) -> bool {
        !matches!(*self, Self::Uninstantiable)
    }

    /// Check if the size of this type is finite (or uninstantiable).
    #[inline]
    pub const fn is_trivial(&self) -> bool {
        matches!(*self, Self::Uninstantiable | Self::Finite(_))
    }

    /// Assume that this is `Max::Finite(..)` and
    /// extract the finite value if so, panicking otherwise.
    /// # Panics
    /// If this value is not `Max::Finite(..)`.
    #[inline]
    #[expect(clippy::panic, reason = "intentional")]
    pub const fn unwrap_finite_ref(&self) -> &Finite {
        match *self {
            Self::Finite(ref finite) => finite,
            Self::Uninstantiable | Self::Infinite => {
                panic!(
                    "Expected `Max::Finite(..)` but found another variant (which can't be printed in a `const fn`)"
                )
            }
        }
    }
}

impl Max<MaybeOverflow<usize>> {
    /// Maximum size of `(Self, Other)`, where `rhs` is the (finite) maximum size of `Other`.
    #[inline]
    #[must_use]
    pub const fn cartesian_product(&self, rhs: usize) -> Self {
        match *self {
            Self::Uninstantiable => Self::Uninstantiable,
            Self::Infinite => Self::Infinite,
            Self::Finite(lhs) => Self::Finite(lhs.plus(rhs)),
        }
    }

    /// Maximum size of `(Self, Other)`, where `rhs` is the maximum size of `Other`.
    #[inline]
    #[must_use]
    pub const fn cartesian_product_with_self(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (&Self::Uninstantiable, _) | (_, &Self::Uninstantiable) => Self::Uninstantiable,
            (&Self::Infinite, _) | (_, &Self::Infinite) => Self::Infinite,
            (&Self::Finite(ref lhs), &Self::Finite(rhs)) => Self::Finite(lhs.plus_self(rhs)),
        }
    }

    #[inline]
    #[must_use]
    pub const fn subtract_from(&self, lhs: usize) -> usize {
        match *self {
            Self::Uninstantiable => lhs,
            Self::Infinite => 0,
            Self::Finite(rhs) => rhs.subtract_from(lhs),
        }
    }
}

impl MaybeDecidable<Max<MaybeOverflow<usize>>> {
    #[inline]
    #[must_use]
    pub const fn cartesian_product(&self, rhs: usize) -> Self {
        match *self {
            Self::Decidable(ref lhs) => Self::Decidable(lhs.cartesian_product(rhs)),
            Self::AtMost(ref lhs) => Self::AtMost(lhs.cartesian_product(rhs)),
        }
    }

    #[inline]
    #[must_use]
    pub const fn cartesian_product_with_self(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (&Self::Decidable(ref lhs), &Self::Decidable(ref rhs)) => {
                Self::Decidable(lhs.cartesian_product_with_self(rhs))
            }
            _ => Self::AtMost(self.at_most().cartesian_product_with_self(rhs.at_most())),
        }
    }

    #[inline]
    #[must_use]
    pub const fn subtract_from(&self, lhs: usize) -> usize {
        self.at_most().subtract_from(lhs)
    }
}

impl Max<f32> {
    #[inline]
    #[must_use]
    pub const fn cartesian_product_with_self(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (&Self::Uninstantiable, _) | (_, &Self::Uninstantiable) => Self::Uninstantiable,
            (&Self::Infinite, _) | (_, &Self::Infinite) => Self::Infinite,
            (&Self::Finite(lhs), &Self::Finite(rhs)) => Self::Finite(lhs + rhs),
        }
    }
}

impl MaybeDecidable<Max<f32>> {
    #[inline]
    #[must_use]
    pub const fn cartesian_product_with_self(&self, rhs: &Self) -> Self {
        match (self, rhs) {
            (&Self::Decidable(ref lhs), &Self::Decidable(ref rhs)) => {
                Self::Decidable(lhs.cartesian_product_with_self(rhs))
            }
            _ => Self::AtMost(self.at_most().cartesian_product_with_self(rhs.at_most())),
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take years")]
impl<Finite: PartialOrd> Eq for Max<Finite> {}

#[expect(clippy::missing_trait_methods, reason = "would take years")]
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

#[expect(clippy::missing_trait_methods, reason = "would take years")]
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

#[expect(clippy::missing_trait_methods, reason = "would take years")]
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
    /// If this turned out to be decidable, return the decided value;
    /// if not, return the maximum possible value.
    #[inline]
    pub const fn at_most(&self) -> &T {
        match *self {
            Self::Decidable(ref at_most) | Self::AtMost(ref at_most) => at_most,
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take years")]
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

#[expect(clippy::missing_trait_methods, reason = "would take years")]
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

#[expect(clippy::missing_trait_methods, reason = "would take years")]
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

#[expect(clippy::missing_trait_methods, reason = "would take years")]
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
