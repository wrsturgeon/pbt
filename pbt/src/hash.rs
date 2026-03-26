use ahash::{HashMap, HashSet, RandomState};

/// The 128-bit hash seed, to be truncated for various platforms.
const SEED_U128: u128 = 0x_1337_1337_1337_1337_1337_1337_1337_1337;
/// The seed with which to initialize hash-related states.
#[expect(clippy::as_conversions, reason = "truncation is fine")]
const SEED: usize = SEED_U128 as usize;

pub type Set<T> = HashSet<T>;
pub type Map<K, V> = HashMap<K, V>;

/// Deterministically initialize a new `ahash` state.
#[inline]
#[must_use]
fn new_state() -> RandomState {
    RandomState::with_seed(SEED)
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
