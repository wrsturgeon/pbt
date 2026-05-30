//! Standardized hash collections.

use ahash::{HashMap, HashSet, RandomState};

/// Initialize a *deterministic* pseudorandom state.
///
/// This pseudorandom state is initialized in exactly the same way each time,
/// without e.g. `getrandom` or any other external source of seeds.
#[inline]
#[must_use]
pub(crate) const fn random_state() -> RandomState {
    // Determinism is more important than cryptographic quality;
    // after all, this is merely a testing library, not a DOS target.
    #[expect(clippy::unusual_byte_groupings, reason = "readability")]
    RandomState::with_seeds(
        0xBAAD_5EED_BAAD_C0DE,
        0xC0DE_CAFE_DECAF_BAD,
        0xDEFEC8ED__BAAD_D00D,
        0x1337_1337_1337_1337,
    )
}

/// Initialize an empty hash map.
#[inline]
#[must_use]
pub const fn map<K, V>() -> HashMap<K, V> {
    HashMap::with_hasher(random_state())
}

/// Initialize an empty hash set.
#[inline]
#[must_use]
pub const fn set<T>() -> HashSet<T> {
    HashSet::with_hasher(random_state())
}
