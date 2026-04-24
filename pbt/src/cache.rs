use {
    crate::{
        SEED,
        construct::{Construct, Decomposition, ElimFn, arbitrary},
        multiset::Multiset,
        reflection::{
            AlgebraicTypeFormer, Erased, ErasedTermBuckets, PrecomputedTypeFormer, Type, info,
            info_by_id,
        },
        size::Size,
    },
    core::{
        any::type_name,
        iter, mem,
        num::NonZero,
        sync::atomic::{AtomicU64, Ordering},
    },
    serde::{Deserialize, Serialize},
    std::{
        collections::BTreeMap,
        env,
        ffi::OsString,
        fs::{self, File, OpenOptions},
        io,
        os::fd::AsRawFd as _,
        path::{Path, PathBuf},
        process,
    },
    wyrand::WyRand,
};

/// Environment variable enabling persistent witness caching.
const ENV_VAR: &str = "PBT_CACHE_DIR";
/// Test-only delay hook used to widen store races in regression tests.
const STORE_DELAY_ENV_VAR: &str = "PBT_TEST_CACHE_STORE_DELAY_MS";

/// Distinguish temporary rewrite paths created by concurrent writers in one process.
static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[expect(clippy::exhaustive_enums, reason = "cache format")]
pub enum CachedTerm {
    Algebraic {
        ctor_idx: NonZero<usize>,
        /// Immediate erased term buckets keyed by the pretty-printed concrete type name.
        term_buckets: BTreeMap<String, Vec<CachedTerm>>,
    },
    Literal(String),
}

/// One line in the per-type NDJSON cache file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CacheLine {
    /// The cached witness term.
    term: CachedTerm,
    /// Fully qualified type name of the top-level witness.
    #[serde(rename = "type")]
    ty: String,
}

#[inline]
#[must_use]
/// Serialize one witness term into the stable cache format.
pub fn serialize_term<T: Construct>(t: &T) -> CachedTerm {
    let info = info::<T>();
    match info.type_former {
        PrecomputedTypeFormer::Literal { serialize, .. } => {
            // SAFETY: Undoing an earlier `transmute` keyed by the surrounding concrete `T`.
            let serialize = unsafe {
                mem::transmute::<for<'t> fn(&'t Erased) -> String, for<'t> fn(&'t T) -> String>(
                    serialize,
                )
            };
            CachedTerm::Literal(serialize(t))
        }
        PrecomputedTypeFormer::Algebraic(AlgebraicTypeFormer { eliminator, .. }) => {
            // SAFETY: Undoing an earlier `transmute` keyed by the surrounding concrete `T`.
            let eliminator = unsafe { mem::transmute::<ElimFn<Erased>, ElimFn<T>>(eliminator) };
            serialize_decomposition(eliminator(t.clone()))
        }
    }
}

#[inline]
#[must_use]
/// Deserialize one cached witness term back into its concrete Rust type.
pub fn deserialize_term<T: Construct>(term: &CachedTerm) -> Option<T> {
    let info = info::<T>();
    match info.type_former {
        PrecomputedTypeFormer::Literal { deserialize, .. } => {
            // SAFETY: Undoing an earlier `transmute` keyed by the surrounding concrete `T`.
            let deserialize = unsafe {
                mem::transmute::<fn(&str) -> Option<Erased>, fn(&str) -> Option<T>>(deserialize)
            };
            let CachedTerm::Literal(ref payload) = *term else {
                return None;
            };
            deserialize(payload)
        }
        PrecomputedTypeFormer::Algebraic(AlgebraicTypeFormer {
            ref all_constructors,
            ..
        }) => {
            let CachedTerm::Algebraic {
                ctor_idx,
                ref term_buckets,
            } = *term
            else {
                return None;
            };
            let &(ctor, ref ctor_meta) = all_constructors.get(ctor_idx.get().checked_sub(1)?)?;
            let mut terms = deserialize_terms(&ctor_meta.constructor.immediate, term_buckets)?;
            // SAFETY: Undoing an earlier `transmute` keyed by the constructor-selected `T`.
            let constructed = unsafe { ctor.unerase::<T>() }(&mut terms)?;
            if !terms.is_empty() {
                return None;
            }
            Some(constructed)
        }
    }
}

#[inline]
#[must_use]
/// Serialize one algebraic decomposition into constructor-tagged cached buckets.
pub fn serialize_decomposition(decomposition: Decomposition) -> CachedTerm {
    let Decomposition { ctor_idx, fields } = decomposition;
    CachedTerm::Algebraic {
        ctor_idx,
        term_buckets: serialize_terms(fields),
    }
}

#[inline]
#[must_use]
#[expect(
    clippy::missing_panics_doc,
    clippy::panic,
    reason = "internal decomposition invariants should panic if violated"
)]
/// Serialize erased term buckets keyed by concrete type into the cache format.
pub fn serialize_terms(mut terms: ErasedTermBuckets) -> BTreeMap<String, Vec<CachedTerm>> {
    let bucket_keys: Vec<_> = terms
        .map
        .keys()
        .copied()
        .map(|ty| (info_by_id(ty).name, ty))
        .collect();
    let buckets = bucket_keys
        .into_iter()
        .map(|(name, ty)| {
            let count = terms
                .map
                .get(&ty)
                .unwrap_or_else(|| panic!("internal `pbt` error: missing term bucket"))
                .terms
                .len();
            let serialized_terms = iter::repeat_with(|| {
                terms.pop_serialize_by_id(ty).unwrap_or_else(|| {
                    panic!("internal `pbt` error: missing serialized term in bucket")
                })
            })
            .take(count)
            .collect();
            (name.to_owned(), serialized_terms)
        })
        .collect();
    assert!(
        terms.is_empty(),
        "internal `pbt` error: leftover terms after serializing buckets: {terms:#?}",
    );
    buckets
}

#[inline]
#[must_use]
/// Rebuild erased term buckets from cached constructor arguments.
pub fn deserialize_terms(
    expected: &Multiset<Type>,
    term_buckets: &BTreeMap<String, Vec<CachedTerm>>,
) -> Option<ErasedTermBuckets> {
    if expected.iter().count() != term_buckets.len() {
        return None;
    }
    let mut terms = ErasedTermBuckets::new();
    for (&ty, count) in expected.iter() {
        let bucket = term_buckets.get(info_by_id(ty).name)?;
        if bucket.len() != count.get() {
            return None;
        }
        for subterm in bucket.iter().rev() {
            if !(info_by_id(ty).deserialize_cached_term_into_buckets)(subterm, &mut terms) {
                return None;
            }
        }
    }
    Some(terms)
}

#[inline]
#[expect(
    clippy::missing_panics_doc,
    clippy::panic,
    reason = "round-trip test helper should panic on mismatch"
)]
pub fn check_roundtrip<T: Construct>() {
    let mut prng = WyRand::new(u64::from(SEED));
    for size in Size::expanding().take(32) {
        let Some(original) = arbitrary::<T>(&mut prng, size) else {
            continue;
        };
        let encoded = serialize_term(&original);
        let decoded = deserialize_term::<T>(&encoded).unwrap_or_else(|| {
            panic!(
                "failed to deserialize round-trip for `{}` from {encoded:#?}",
                type_name::<T>(),
            )
        });
        assert_eq!(
            decoded,
            original,
            "round-trip mismatch for `{}`",
            type_name::<T>()
        );
    }
}

#[inline]
#[must_use]
/// Load all cached witnesses currently stored for one top-level witness type.
pub fn load<T: Construct>() -> Vec<T> {
    let path = cache_path::<T>();
    let Ok(contents) = fs::read_to_string(path) else {
        return vec![];
    };
    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<CacheLine>(line).ok())
        .filter(|line| line.ty == type_name::<T>())
        .filter_map(|line| deserialize_term::<T>(&line.term))
        .collect()
}

#[inline]
/// Store one witness in the persistent per-type cache, keeping canonical ordering.
pub fn store<T: Construct>(t: &T) {
    let path = cache_path::<T>();
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let serialized = serialize_term(t);
    let Ok(()) = with_cache_lock(&path, || store_locked::<T>(&path, &serialized)) else {
        return;
    };
}

/// Rewrite a cache file after the caller has acquired the per-file cache lock.
#[inline]
fn store_locked<T: Construct>(path: &Path, term: &CachedTerm) -> io::Result<()> {
    let mut witnesses = load::<T>()
        .into_iter()
        .map(|witness| serialize_term(&witness))
        .collect::<Vec<_>>();
    maybe_test_store_delay();
    witnesses.push(term.clone());
    canonicalize_terms(&mut witnesses);
    let encoded = witnesses
        .into_iter()
        .map(|term| CacheLine {
            term,
            ty: type_name::<T>().to_owned(),
        })
        .map(|line| serde_json::to_string(&line))
        .collect::<Result<Vec<_>, _>>();
    let lines = encoded.map_err(io::Error::other)?;
    let body = if lines.is_empty() {
        String::new()
    } else {
        let mut body = lines.join("\n");
        body.push('\n');
        body
    };
    atomic_write(path, &body)
}

/// Hold an exclusive sibling lock file while running a cache read-modify-write step.
#[inline]
fn with_cache_lock<R>(path: &Path, f: impl FnOnce() -> io::Result<R>) -> io::Result<R> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(lock_path(path))?;
    lock_exclusive(&lock)?;
    let result = f();
    let unlock_result = unlock(&lock);
    match (result, unlock_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), _) | (Ok(_), Err(error)) => Err(error),
    }
}

/// Compute the sibling lock-file path for a cache file.
#[inline]
fn lock_path(path: &Path) -> PathBuf {
    let mut lock_path = OsString::from(path.as_os_str());
    lock_path.push(".lock");
    PathBuf::from(lock_path)
}

/// Acquire an exclusive advisory lock on an already-open lock file.
#[inline]
fn lock_exclusive(file: &File) -> io::Result<()> {
    let fd = file.as_raw_fd();
    // SAFETY: `fd` is a live file descriptor for the duration of this call.
    let result = unsafe { libc::flock(fd, libc::LOCK_EX) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Release an advisory lock previously acquired by [`lock_exclusive`].
#[inline]
fn unlock(file: &File) -> io::Result<()> {
    let fd = file.as_raw_fd();
    // SAFETY: `fd` is a live file descriptor for the duration of this call.
    let result = unsafe { libc::flock(fd, libc::LOCK_UN) };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

/// Atomically rewrite a cache file via a temporary sibling path.
#[inline]
fn atomic_write(path: &Path, body: &str) -> io::Result<()> {
    let tmp = path.with_extension(format!(
        "{}.{}.tmp",
        process::id(),
        TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&tmp, body)?;
    fs::rename(tmp, path)
}

/// Delay a cache store during tests so inter-process races can be reproduced reliably.
#[inline]
fn maybe_test_store_delay() {
    use {core::time::Duration, std::thread};

    if !cfg!(test) {
        return;
    }
    let Ok(delay_ms) = env::var(STORE_DELAY_ENV_VAR) else {
        return;
    };
    let Ok(delay_ms) = delay_ms.parse::<u64>() else {
        return;
    };
    thread::sleep(Duration::from_millis(delay_ms));
}

/// Compute the NDJSON cache path for a top-level witness type.
#[inline]
fn cache_path<T: Construct>() -> PathBuf {
    let root = env::var_os(ENV_VAR).map_or_else(default_cache_root, PathBuf::from);
    let ty = type_name::<T>();
    let hash = fnv1a64(ty.as_bytes());
    root.join("witnesses")
        .join(format!("{hash:016x}--{}.ndjson", slugify(ty)))
}

/// Sort serialized witnesses into a canonical smallest-first order and drop duplicates.
#[inline]
fn canonicalize_terms(terms: &mut Vec<CachedTerm>) {
    terms.sort_unstable_by(|lhs, rhs| {
        witness_size(lhs)
            .cmp(&witness_size(rhs))
            .then_with(|| lhs.cmp(rhs))
    });
    terms.dedup();
}

/// Count the total number of serialized witness nodes in a decomposition tree.
#[inline]
#[must_use]
fn witness_size(term: &CachedTerm) -> usize {
    match *term {
        CachedTerm::Literal(_) => 1,
        CachedTerm::Algebraic {
            ref term_buckets, ..
        } => {
            let child_nodes = term_buckets
                .values()
                .flatten()
                .map(witness_size)
                .sum::<usize>();
            child_nodes.saturating_add(1)
        }
    }
}

/// Resolve the default persistent cache root for this crate or workspace.
#[inline]
fn default_cache_root() -> PathBuf {
    default_cache_base().join(".pbt")
}

/// Resolve the project root used for the default persistent cache.
#[inline]
fn default_cache_base() -> PathBuf {
    let Ok(current_dir) = env::current_dir() else {
        return PathBuf::from(".");
    };
    current_dir
        .ancestors()
        .find(|candidate| candidate.join(".git").exists() || candidate.join("target").is_dir())
        .map(Path::to_path_buf)
        .unwrap_or(current_dir)
}

/// Deterministic FNV-1a hash used to stabilize per-type cache filenames.
#[inline]
fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

/// Make a short human-readable suffix for the cache filename.
#[inline]
fn slugify(s: &str) -> String {
    let slug: String = s
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .take(48)
        .collect();
    slug.trim_matches('-').to_owned()
}

#[cfg(test)]
mod test {
    use {
        super::*,
        crate::{
            construct::Construct,
            sigma::{Predicate, Sigma},
        },
        core::{convert::Infallible, num::NonZero},
        std::{
            collections::{BTreeMap, BTreeSet, HashMap, HashSet},
            ffi::CString,
            rc::Rc,
            sync::Arc,
        },
    };

    enum NotTheAnswer {}

    impl Predicate<u8> for NotTheAnswer {
        type Error = String;

        #[inline]
        fn check(candidate: &u8) -> Result<(), Self::Error> {
            if *candidate == 42 {
                Err(format!(
                    "The Answer to the Ultimate Question of Life, the Universe, and Everything is {candidate}",
                ))
            } else {
                Ok(())
            }
        }
    }

    type NonAnswer = Sigma<u8, NotTheAnswer>;
    fn roundtrip<T: Construct>() {
        check_roundtrip::<T>();
    }

    #[test]
    fn roundtrip_literals_and_builtins() {
        roundtrip::<bool>();
        roundtrip::<u64>();
        roundtrip::<char>();
        roundtrip::<NonZero<u8>>();
        roundtrip::<Box<bool>>();
        roundtrip::<Option<u64>>();
        roundtrip::<Vec<u64>>();
        roundtrip::<BTreeSet<u64>>();
        roundtrip::<BTreeMap<u64, u64>>();
        roundtrip::<HashSet<u64>>();
        roundtrip::<HashMap<u64, u64>>();
        roundtrip::<Rc<bool>>();
        roundtrip::<Arc<bool>>();
        roundtrip::<(u64, u64)>();
        roundtrip::<String>();
        roundtrip::<CString>();
        roundtrip::<NonAnswer>();
        roundtrip::<Infallible>();
    }
}
