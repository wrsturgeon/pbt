use core::cmp;

#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeInstantiable<Instantiable> {
    Instantiable(Instantiable),
    Uninstantiable,
}

#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeInfinite<Finite> {
    Finite(Finite),
    Infinite,
}

#[derive(Debug)]
#[expect(clippy::exhaustive_enums, reason = "Nope, this is it.")]
pub enum MaybeOverflow<Contained> {
    Contained(Contained),
    Overflow,
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
