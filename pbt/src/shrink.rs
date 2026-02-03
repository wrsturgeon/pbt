use {core::hash::Hash, std::collections::HashSet};

pub trait Shrink: Clone + Eq + Hash {
    #[must_use]
    fn step<P: for<'s> FnMut(&'s Self) -> bool>(&self, property: &mut P) -> Option<Self>;
}

#[inline]
#[must_use]
pub fn minimal<T: Shrink, P: for<'t> Fn(&'t T) -> bool>(t: &T, property: P) -> T {
    let mut reuse = HashSet::new();
    let mut property = move |t: &T| {
        if reuse.contains(t) {
            false
        } else {
            reuse.insert(t.clone()) && property(t)
        }
    };
    let Some(mut acc) = t.step(&mut property) else {
        return t.clone();
    };
    while let Some(reduced) = acc.step(&mut property) {
        acc = reduced;
    }
    acc
}

/*
/// Check that a shrinking function works as intended for a single input.
/// # Panics
/// If that's not the case.
#[inline]
#[expect(clippy::panic, reason = "failing tests ought to panic")]
fn check_shrinking_once<T: fmt::Debug + Ord + Shrink>(minimal: &T, greater: &T) {
    let shrunk = greater.shrink(|t: &T| {
        print!("{t:?}");
        let greater = *t >= *minimal;
        println!(" {}", if greater { 'Y' } else { 'N' });
        greater
    });
    pretty_assertions::assert_eq!(
        *minimal,
        shrunk,
        "{greater:?} shrunk to {shrunk:?}, but it should have shrunk further to {minimal:?}",
    );
}

/// Check that a shrinking function works as intended
/// for a wide range of possible values.
/// # Panics
/// If a counterexample shows up.
#[inline]
#[expect(clippy::unwrap_used, reason = "failing tests ought to panic")]
pub fn check_shrinking<T: Conjure + fmt::Debug + Ord + Shrink>() {
    const N_TRIALS: usize = 1_000;

    let mut seed = Seed::new();
    for (mut minimal, mut greater) in <(T, T)>::corners() {
        if minimal > greater {
            let () = mem::swap(&mut minimal, &mut greater);
        }
        let () = check_shrinking_once(&minimal, &greater);
    }
    for size in 0..N_TRIALS {
        let (mut minimal, mut greater) = <(T, T)>::conjure(seed.split(), size).unwrap();
        if minimal > greater {
            let () = mem::swap(&mut minimal, &mut greater);
        }
        let () = check_shrinking_once(&minimal, &greater);
    }
}
*/
