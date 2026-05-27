#![expect(
    clippy::arbitrary_source_item_ordering,
    reason = "macros are parsed top-to-bottom"
)]

//! Cache an *idempotent* calculation with an `RwLock<HashMap<K, V>>`.

/// Cache an *idempotent* calculation with an `RwLock<HashMap<K, V>>`.
macro_rules! memoize {
    ($name:literal = |$k:ident: $K:ty| -> $V:ty $b:block) => {{
        static CACHE: RwLock<HashMap<$K, $V>> = RwLock::new(map());

        // Check if this input already has a cached result:
        {
            let read = CACHE.read().expect(concat!(
                "INTERNAL ERROR (`pbt`): ",
                $name,
                " cache lock poisoned",
            ));
            if let Some(cached) = read.get(&$k) {
                return <$V as Clone>::clone(cached);
            }
        }

        // Otherwise, compute the result and insert it,
        // unless there was a race condition (in which case
        // it's important that this function be idempotent):
        let v = $b;
        let mut write = CACHE.write().expect(concat!(
            "INTERNAL ERROR (`pbt`): ",
            $name,
            " cache lock poisoned",
        ));
        <$V as Clone>::clone(write.entry($k).or_insert(v))
    }};
}

pub(crate) use memoize;
