//! Property-based testing with `derive`, aware of uninstantiable and inductive types.

extern crate alloc;

pub mod conjure;
pub mod count;
pub mod shrink;

mod impls;

pub use ::pbt_macros::Pbt;

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NotFound;

/// Attempt to produce a constructive witness of a computable predicate.
/// # Errors
/// If one could not be found in time.
/// Note that this does *not* mean that one does not exist.
#[inline]
pub fn witness<T: conjure::Conjure + shrink::Shrink, P: Fn(&T) -> bool>(
    predicate: P,
) -> Result<T, NotFound> {
    const N_TRIALS: usize = 1_000;

    for seed in conjure::seeds().take(N_TRIALS) {
        let Ok(witness) = <T as conjure::Conjure>::conjure(seed) else {
            return Err(NotFound);
        };
        if predicate(&witness) {
            return Ok(shrink::minimal(&witness, predicate));
        }
    }
    Err(NotFound)
}

#[cfg(test)]
#[expect(
    clippy::print_stdout,
    clippy::use_debug,
    reason = "failing tests ought to panic and be debugged"
)]
mod test {
    use super::*;

    #[test]
    fn witness_42_exists() {
        let witness = witness(|&u: &u8| {
            print!("{u:?}");
            let success = u == 42;
            println!(" {}", if success { 'Y' } else { 'N' });
            success
        });
        let () = pretty_assertions::assert_eq!(Ok(42), witness);
    }

    #[test]
    fn witness_at_least_42() {
        let witness = witness(|&u: &u8| {
            print!("{u:?}");
            let success = u >= 42;
            println!(" {}", if success { 'Y' } else { 'N' });
            success
        });
        let () = pretty_assertions::assert_eq!(Ok(42), witness);
    }

    #[test]
    fn witness_vec_of_at_least_3_elements() {
        let witness = witness(|v: &Vec<i8>| {
            print!("{v:?}");
            let success = v.len() >= 3;
            println!(" {}", if success { 'Y' } else { 'N' });
            success
        });
        let () = pretty_assertions::assert_eq!(Ok(vec![0, 0, 0]), witness);
    }
}
