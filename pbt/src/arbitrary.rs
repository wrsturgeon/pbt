//! Generate an arbitrary term of some type.

use {
    crate::{pbt::Pbt, size::Size, swarm::Swarm},
    wyrand::WyRand,
};

/// Generate an arbitrary term of some type.
#[inline]
#[expect(clippy::todo, reason = "TODO")]
pub fn arbitrary<T>(_swarm: &mut Swarm, _size: Size, _prng: &mut WyRand) -> T
where
    T: Pbt,
{
    todo!()
}
