use ahash::{HashMap, HashSet, RandomState};

/// The 16-bit hash seed, to be zero-extended for various platforms.
pub(crate) const SEED: u16 = 0x1337;

pub type Set<T> = HashSet<T>;
pub type Map<K, V> = HashMap<K, V>;

/// Deterministically initialize a new `ahash` state.
#[inline]
#[must_use]
fn new_state() -> RandomState {
    RandomState::with_seed(usize::from(SEED))
}

/// Deterministically initialize an empty set.
#[inline]
#[must_use]
pub fn empty_set<T>() -> Set<T> {
    Set::with_hasher(new_state())
}

/// Deterministically initialize an empty map.
#[inline]
#[must_use]
pub fn empty_map<K, V>() -> Map<K, V> {
    Map::with_hasher(new_state())
}
