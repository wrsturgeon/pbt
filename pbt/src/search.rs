use {
    crate::{
        SEED, cache,
        construct::{Construct, arbitrary},
        shrink::shrink,
        size::Size,
    },
    core::fmt,
    std::env,
    wyrand::WyRand,
};

#[derive(Clone, Copy, Debug)]
#[expect(clippy::exhaustive_structs, reason = "genuinely exhaustive")]
pub struct Named<T> {
    pub name: &'static str,
    pub value: T,
}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl<T: Eq> Eq for Named<T> {}

#[expect(clippy::missing_trait_methods, reason = "intentionally left default")]
impl<T: PartialEq> PartialEq for Named<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        T::eq(&self.value, &other.value)
    }
}

#[inline]
pub fn witness<T: Construct, P: Fn(&T) -> bool>(n_cases: usize, property: P) -> Option<T> {
    let mut prng = WyRand::new(
        env::var("PBT_SEED")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| getrandom::u64().unwrap_or_else(|_| u64::from(SEED))),
    );
    for maybe_t in cache::load::<T>().into_iter().map(Some).chain(
        Size::expanding()
            .take(n_cases)
            .map(|size| arbitrary::<T>(&mut prng, size)),
    ) {
        let t = maybe_t?;
        if property(&t) {
            let mut best_yet = t;
            'restart: loop {
                for candidate in shrink::<T>(best_yet.clone()) {
                    if property(&candidate) {
                        best_yet = candidate;
                        continue 'restart;
                    }
                }
                // nothing better, so return our best-yet as the best overall:
                cache::store(&best_yet);
                return Some(best_yet);
            }
        }
    }
    None
}

/// Assert that some property always holds (i.e. returns `true`).
/// # Panics
/// If a minimal witness can be found
/// by checking up to `n_cases` cases
/// for which the property returns `false`.
#[inline]
#[expect(clippy::panic, reason = "failing assertions ought to panic")]
pub fn assert<T: Construct, P: Fn(&T) -> bool>(n_cases: usize, property: P) {
    if let Some(t) = witness(n_cases, |t| !property(t)) {
        assert!(
            property(&t),
            "\r\n\r\nnot always true: for example, consider the input\r\n{t:#?}\r\n\r\n",
        );
        panic!(
            "\r\n\r\nflaky test! the input\r\n{t:#?}\r\noriginally returned `{:?}`, but when run again, it returned `{:?}`\r\n\r\n",
            false, true,
        )
    }
}

/// Assert that two terms (which may vary
/// depending on some input) are always equal.
/// # Panics
/// If a minimal witness can be found
/// by checking up to `n_cases` cases
/// for which the two terms differ.
#[inline]
#[expect(clippy::panic, reason = "failing assertions ought to panic")]
pub fn assert_eq<X: Construct, Y: fmt::Debug + Eq, P: Fn(&X) -> (Y, Y)>(
    n_cases: usize,
    property: P,
) {
    if let Some(x) = witness(n_cases, |x| {
        let (lhs, rhs) = property(x);
        lhs != rhs
    }) {
        let (lhs, rhs) = property(&x);
        pretty_assertions::assert_eq!(
            lhs,
            rhs,
            "\r\n\r\nnot always equal: for example, consider the input\r\n{x:#?}\r\n\r\n",
        );
        panic!(
            "\r\n\r\nflaky test! the input\r\n{x:#?}\r\noriginally failed, but when run again, it produced\r\n{lhs:#?}\r\nand\r\n{rhs:#?}\r\nwhich were judged to be equal\r\n\r\n",
        )
    }
}
