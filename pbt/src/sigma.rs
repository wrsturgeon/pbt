//! Sigma-types: types whose terms satisfy a predicate
//! (e.g. floats greater than zero).

#![expect(clippy::missing_trait_methods, reason = "left intentionally default")]

use {
    crate::{
        pbt::{
            Algebraic, CtorFn, Decomposition, ElimFn, IntroductionRule, Pbt, TypeFormer,
            push_arbitrary_field, visit_self,
        },
        reflection::{TermsOfVariousTypes, Type, register, type_of},
        scc::StronglyConnectedComponents,
    },
    core::{
        cmp, fmt,
        hash::{Hash, Hasher},
        iter,
        marker::PhantomData,
        num::NonZero,
        ops::Deref,
    },
    std::collections::BTreeSet,
};

/// A runtime-decidable predicate on some type.
pub trait Predicate<T>: 'static {
    /// Printable error iff `check` fails.
    type Error: fmt::Display;
    /// Decide whether an arbitrary candidate satisfies this predicate.
    /// # Errors
    /// If this predicate does not hold.
    fn check(candidate: &T) -> Result<(), Self::Error>;
}

/// Sigma-types: types whose terms satisfy a predicate
/// (e.g. floats greater than zero).
#[repr(transparent)]
#[expect(clippy::partial_pub_fields, reason = "fine")]
pub struct Sigma<T, P: Predicate<T>> {
    /// The predicate that `self.value` satisfies.
    _predicate: PhantomData<P>,
    /// A value of type `T` that additionally
    /// satisfies the predicate `P`.
    pub value: T,
}

impl<T, P: Predicate<T>> Sigma<T, P> {
    /// Attempt to create a new term of this Sigma-type
    /// by checking the predicate and succeeding iff the predicate holds.
    /// # Errors
    /// If the property does not hold for the candidate provided.
    #[inline]
    pub fn new(candidate: T) -> Result<Self, (P::Error, T)> {
        match P::check(&candidate) {
            Ok(()) => Ok(Self {
                _predicate: PhantomData,
                value: candidate,
            }),
            Err(e) => Err((e, candidate)),
        }
    }

    /// Attempt to create a new term of this Sigma-type
    /// by checking the predicate and succeeding iff the predicate holds.
    /// # Safety
    /// Nonsensical value if the property does not hold for the candidate provided.
    /// # Panics
    /// If debug assertions are enabled and `P::check` fails.
    #[inline]
    pub unsafe fn new_unchecked(candidate: T) -> Self {
        #[cfg(debug_assertions)]
        {
            match P::check(&candidate) {
                Ok(()) => {}
                #[expect(clippy::panic, reason = "better than unsafe values")]
                Err(e) => panic!("{e}"),
            }
        }
        Self {
            _predicate: PhantomData,
            value: candidate,
        }
    }
}

impl<T: Clone, P: Predicate<T>> Clone for Sigma<T, P> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self {
            _predicate: PhantomData,
            value: <T as Clone>::clone(&self.value),
        }
    }
}

impl<T: Copy, P: Predicate<T>> Copy for Sigma<T, P> {}

impl<T, P: Predicate<T>> Deref for Sigma<T, P> {
    type Target = T;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T: Eq, P: Predicate<T>> Eq for Sigma<T, P> {}

impl<T: Hash, P: Predicate<T>> Hash for Sigma<T, P> {
    #[inline(always)]
    fn hash<H: Hasher>(&self, state: &mut H) {
        <T as Hash>::hash(&self.value, state)
    }
}

impl<T: Ord, P: Predicate<T>> Ord for Sigma<T, P> {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        <T as Ord>::cmp(&self.value, &other.value)
    }
}

impl<T: PartialEq, P: Predicate<T>> PartialEq for Sigma<T, P> {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        <T as PartialEq>::eq(&self.value, &other.value)
    }
}

impl<T: PartialOrd, P: Predicate<T>> PartialOrd for Sigma<T, P> {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        <T as PartialOrd>::partial_cmp(&self.value, &other.value)
    }
}

impl<T: fmt::Debug, P: Predicate<T>> fmt::Debug for Sigma<T, P> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <T as fmt::Debug>::fmt(&self.value, f)
    }
}

impl<T: fmt::Display, P: Predicate<T>> fmt::Display for Sigma<T, P> {
    #[inline(always)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <T as fmt::Display>::fmt(&self.value, f)
    }
}

// TODO: how should this behave when e.g.
// we're asking for a non-empty `Vec` of size `0`
// (since that's impossible and will loop forever)?
impl<T: Pbt, P: Predicate<T>> Pbt for Sigma<T, P> {
    #[inline]
    fn register_all_immediate_dependencies(
        visited: &mut BTreeSet<Type>,
        sccs: &mut StronglyConnectedComponents,
    ) {
        if !visited.insert(type_of::<Self>()) {
            return;
        }
        let () = register::<T>(visited.clone(), sccs);
    }

    #[inline]
    fn type_former() -> TypeFormer<Self> {
        TypeFormer::Algebraic(Algebraic {
            elimination_rule: ElimFn::new(|Self { value, .. }| {
                let mut fields = TermsOfVariousTypes::new();
                let () = fields.push::<T>(value);
                Decomposition {
                    ctor_idx: const { NonZero::new(1).unwrap() },
                    fields,
                }
            }),
            introduction_rules: vec![IntroductionRule {
                arbitrary_fields: |prng, mut sizes| {
                    let mut fields = TermsOfVariousTypes::new();
                    push_arbitrary_field::<T>(&mut fields, &mut sizes, prng)?;
                    Ok(fields)
                },
                call: CtorFn {
                    call: |terms| Self::new(terms.must_pop()).ok(),
                },
                immediate_dependencies: iter::once(type_of::<T>()).collect(),
            }],
        })
    }

    #[inline]
    fn visit_deep<V: Pbt>(&self) -> impl Iterator<Item = V> {
        visit_self(self).chain(T::visit_deep(&self.value))
    }
}
