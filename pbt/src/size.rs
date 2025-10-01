use core::cmp;

#[must_use]
#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeInstantiable<Instantiable> {
    Instantiable(Instantiable),
    Uninstantiable,
}

#[must_use]
#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeInfinite<Finite> {
    Finite(Finite),
    Infinite,
}

#[must_use]
#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeOverflow<Contained> {
    Contained(Contained),
    Overflow,
}

impl<X> MaybeInstantiable<X> {
    #[inline]
    pub fn map<Y, F: FnOnce(X) -> Y>(self, map: F) -> MaybeInstantiable<Y> {
        match self {
            Self::Uninstantiable => MaybeInstantiable::Uninstantiable,
            Self::Instantiable(x) => MaybeInstantiable::Instantiable(map(x)),
        }
    }
}

impl MaybeOverflow<usize> {
    #[inline]
    pub const fn plus(self, rhs: usize) -> Self {
        match self {
            Self::Overflow => Self::Overflow,
            Self::Contained(lhs) => match lhs.checked_add(rhs) {
                Some(sum) => Self::Contained(sum),
                None => Self::Overflow,
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Finite: PartialEq> PartialEq for MaybeInstantiable<Finite> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match *self {
            Self::Uninstantiable => match *other {
                Self::Uninstantiable => true,
                Self::Instantiable(..) => false,
            },
            Self::Instantiable(ref lhs) => match *other {
                Self::Uninstantiable => false,
                Self::Instantiable(ref rhs) => lhs.eq(rhs),
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Finite: PartialOrd> PartialOrd for MaybeInstantiable<Finite> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match *self {
            Self::Uninstantiable => match *other {
                Self::Uninstantiable => Some(cmp::Ordering::Equal),
                Self::Instantiable(..) => Some(cmp::Ordering::Less),
            },
            Self::Instantiable(ref lhs) => match *other {
                Self::Uninstantiable => Some(cmp::Ordering::Greater),
                Self::Instantiable(ref rhs) => lhs.partial_cmp(rhs),
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Finite: Eq> Eq for MaybeInstantiable<Finite> {}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Finite: Ord> Ord for MaybeInstantiable<Finite> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match *self {
            Self::Uninstantiable => match *other {
                Self::Uninstantiable => cmp::Ordering::Equal,
                Self::Instantiable(..) => cmp::Ordering::Less,
            },
            Self::Instantiable(ref lhs) => match *other {
                Self::Uninstantiable => cmp::Ordering::Greater,
                Self::Instantiable(ref rhs) => lhs.cmp(rhs),
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Finite: PartialEq> PartialEq for MaybeInfinite<Finite> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match *self {
            Self::Infinite => false,
            Self::Finite(ref lhs) => match *other {
                Self::Infinite => false,
                Self::Finite(ref rhs) => lhs.eq(rhs),
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Finite: PartialOrd> PartialOrd for MaybeInfinite<Finite> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match *self {
            Self::Infinite => match *other {
                Self::Infinite => None,
                Self::Finite(_) => Some(cmp::Ordering::Greater),
            },
            Self::Finite(ref lhs) => match *other {
                Self::Infinite => Some(cmp::Ordering::Less),
                Self::Finite(ref rhs) => lhs.partial_cmp(rhs),
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Contained: PartialEq> PartialEq for MaybeOverflow<Contained> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        match *self {
            Self::Overflow => false,
            Self::Contained(ref lhs) => match *other {
                Self::Overflow => false,
                Self::Contained(ref rhs) => lhs.eq(rhs),
            },
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "would take decades")]
impl<Contained: PartialOrd> PartialOrd for MaybeOverflow<Contained> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match *self {
            Self::Overflow => match *other {
                Self::Overflow => None,
                Self::Contained(_) => Some(cmp::Ordering::Greater),
            },
            Self::Contained(ref lhs) => match *other {
                Self::Overflow => Some(cmp::Ordering::Less),
                Self::Contained(ref rhs) => lhs.partial_cmp(rhs),
            },
        }
    }
}
