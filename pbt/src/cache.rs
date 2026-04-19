use {
    crate::{
        SEED,
        construct::{Construct, Decomposition, ElimFn, arbitrary},
        reflection::{
            AlgebraicTypeFormer, Erased, PrecomputedTypeFormer, TermsOfVariousTypes, info,
            info_by_id,
        },
        size::Size,
    },
    core::{any::type_name, iter, mem, num::NonZero},
    serde::{Deserialize, Serialize},
    std::{
        env, fs, io,
        path::{Path, PathBuf},
        process,
    },
    wyrand::WyRand,
};

/// Current serialized witness schema version.
const FORMAT_VERSION: u8 = 1;
/// Environment variable enabling persistent witness caching.
const ENV_VAR: &str = "PBT_CACHE_DIR";

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[expect(clippy::exhaustive_enums, reason = "cache format")]
pub enum WitnessTerm {
    Literal(String),
    Node {
        ctor_idx: NonZero<usize>,
        fields: Vec<Vec<WitnessTerm>>,
    },
}

/// One line in the per-type NDJSON cache file.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
struct CacheLine {
    /// The cached witness term.
    term: WitnessTerm,
    /// Fully qualified type name of the top-level witness.
    #[serde(rename = "type")]
    ty: String,
    /// Cache format version.
    v: u8,
}

#[inline]
#[must_use]
#[expect(
    clippy::missing_panics_doc,
    clippy::panic,
    reason = "internal decomposition invariants should panic if violated"
)]
pub fn serialize_term<T: Construct>(t: &T) -> WitnessTerm {
    let info = info::<T>();
    match info.type_former {
        PrecomputedTypeFormer::Literal { serialize, .. } => {
            // SAFETY: Undoing an earlier `transmute` keyed by the surrounding concrete `T`.
            let serialize = unsafe {
                mem::transmute::<for<'t> fn(&'t Erased) -> String, for<'t> fn(&'t T) -> String>(
                    serialize,
                )
            };
            WitnessTerm::Literal(serialize(t))
        }
        PrecomputedTypeFormer::Algebraic(AlgebraicTypeFormer {
            ref all_constructors,
            eliminator,
            ..
        }) => {
            // SAFETY: Undoing an earlier `transmute` keyed by the surrounding concrete `T`.
            let eliminator = unsafe { mem::transmute::<ElimFn<Erased>, ElimFn<T>>(eliminator) };
            let Decomposition {
                ctor_idx,
                mut fields,
            } = eliminator(t.clone());
            let idx = ctor_idx
                .get()
                .checked_sub(1)
                .unwrap_or_else(|| panic!("internal `pbt` error: zero constructor index"));
            let Some(&(_, ref ctor)) = all_constructors.get(idx) else {
                panic!("internal `pbt` error: constructor index out of bounds")
            };
            let grouped_fields = ctor
                .constructor
                .immediate
                .iter()
                .map(|(&ty, count)| {
                    iter::repeat_with(|| {
                        fields.pop_serialize_by_id(ty).unwrap_or_else(|| {
                            panic!("internal `pbt` error: missing serialized field")
                        })
                    })
                    .take(count.get())
                    .collect()
                })
                .collect();
            assert!(
                fields.is_empty(),
                "internal `pbt` error: leftover terms after serializing a decomposition: {fields:#?}",
            );
            WitnessTerm::Node {
                ctor_idx,
                fields: grouped_fields,
            }
        }
    }
}

#[inline]
#[must_use]
pub fn deserialize_term<T: Construct>(term: &WitnessTerm) -> Option<T> {
    let info = info::<T>();
    match info.type_former {
        PrecomputedTypeFormer::Literal { deserialize, .. } => {
            // SAFETY: Undoing an earlier `transmute` keyed by the surrounding concrete `T`.
            let deserialize = unsafe {
                mem::transmute::<fn(&str) -> Option<Erased>, fn(&str) -> Option<T>>(deserialize)
            };
            let WitnessTerm::Literal(ref payload) = *term else {
                return None;
            };
            deserialize(payload)
        }
        PrecomputedTypeFormer::Algebraic(AlgebraicTypeFormer {
            ref all_constructors,
            ..
        }) => {
            let WitnessTerm::Node {
                ctor_idx,
                ref fields,
            } = *term
            else {
                return None;
            };
            let &(ctor, ref ctor_meta) = all_constructors.get(ctor_idx.get().checked_sub(1)?)?;
            if ctor_meta.constructor.immediate.iter().count() != fields.len() {
                return None;
            }
            let mut terms = TermsOfVariousTypes::new();
            for ((&ty, count), bucket) in ctor_meta.constructor.immediate.iter().zip(fields) {
                if bucket.len() != count.get() {
                    return None;
                }
                for subterm in bucket.iter().rev() {
                    if !(info_by_id(ty).deserialize_into_terms)(subterm, &mut terms) {
                        return None;
                    }
                }
            }
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
#[expect(
    clippy::missing_panics_doc,
    clippy::panic,
    reason = "round-trip test helper should panic on mismatch"
)]
pub fn check_round_trip<T: Construct>() {
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
pub fn load<T: Construct>() -> Vec<T> {
    let path = cache_path::<T>();
    let Ok(contents) = fs::read_to_string(path) else {
        return vec![];
    };
    contents
        .lines()
        .filter_map(|line| serde_json::from_str::<CacheLine>(line).ok())
        .filter(|line| line.v == FORMAT_VERSION && line.ty == type_name::<T>())
        .filter_map(|line| deserialize_term::<T>(&line.term))
        .collect()
}

#[inline]
pub fn store<T: Construct>(t: &T) {
    let path = cache_path::<T>();
    let mut witnesses = load::<T>();
    if witnesses.iter().any(|cached| cached == t) {
        return;
    }
    witnesses.push(t.clone());
    let Some(parent) = path.parent() else {
        return;
    };
    if fs::create_dir_all(parent).is_err() {
        return;
    }
    let encoded = witnesses
        .into_iter()
        .map(|witness| CacheLine {
            term: serialize_term(&witness),
            ty: type_name::<T>().to_owned(),
            v: FORMAT_VERSION,
        })
        .map(|line| serde_json::to_string(&line))
        .collect::<Result<Vec<_>, _>>();
    let Ok(lines) = encoded else {
        return;
    };
    let body = if lines.is_empty() {
        String::new()
    } else {
        let mut body = lines.join("\n");
        body.push('\n');
        body
    };
    drop(atomic_write(&path, &body));
}

/// Atomically rewrite a cache file via a temporary sibling path.
#[inline]
fn atomic_write(path: &Path, body: &str) -> io::Result<()> {
    let tmp = path.with_extension(format!("{}.tmp", process::id()));
    fs::write(&tmp, body)?;
    fs::rename(tmp, path)
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
            env,
            env::temp_dir,
            ffi::CString,
            rc::Rc,
            sync::Arc,
            sync::{LazyLock, Mutex},
            time::{SystemTime, UNIX_EPOCH},
        },
    };

    static ENV_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

    enum NotTheAnswer {}

    impl Predicate<u8> for NotTheAnswer {
        fn check(candidate: &u8) -> bool {
            *candidate != 42
        }
    }

    type NonAnswer = Sigma<u8, NotTheAnswer>;

    fn round_trip<T: Construct>() {
        check_round_trip::<T>();
    }

    #[test]
    fn round_trip_literals_and_builtins() {
        round_trip::<bool>();
        round_trip::<u64>();
        round_trip::<char>();
        round_trip::<NonZero<u8>>();
        round_trip::<Box<bool>>();
        round_trip::<Option<u64>>();
        round_trip::<Vec<u64>>();
        round_trip::<BTreeSet<u64>>();
        round_trip::<BTreeMap<u64, u64>>();
        round_trip::<HashSet<u64>>();
        round_trip::<HashMap<u64, u64>>();
        round_trip::<Rc<bool>>();
        round_trip::<Arc<bool>>();
        round_trip::<(u64, u64)>();
        round_trip::<String>();
        round_trip::<CString>();
        round_trip::<NonAnswer>();
        round_trip::<Infallible>();
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test setup failures should panic")]
    fn cache_store_and_load() {
        let _guard = ENV_LOCK.lock().expect("test env lock poisoned");

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        let root = temp_dir().join(format!("pbt-cache-test-{unique}"));
        // SAFETY: This test holds a global mutex to serialize environment mutation.
        unsafe {
            env::set_var(ENV_VAR, &root);
        }

        let witness = vec![1_u64, 2, 3];
        store(&witness);
        store(&witness);
        let loaded = load::<Vec<u64>>();
        assert_eq!(loaded, vec![witness.clone()]);

        let path = cache_path::<Vec<u64>>();
        let body = fs::read_to_string(&path).expect("cache file");
        fs::write(&path, format!("not-json\n{body}")).expect("prepend malformed cache line");
        assert_eq!(load::<Vec<u64>>(), vec![witness]);

        // SAFETY: This test holds a global mutex to serialize environment mutation.
        unsafe {
            env::remove_var(ENV_VAR);
        }
        drop(fs::remove_dir_all(root));
    }

    #[test]
    #[expect(clippy::expect_used, reason = "test setup failures should panic")]
    fn cache_path_defaults_to_workspace_dot_pbt() {
        let _guard = ENV_LOCK.lock().expect("test env lock poisoned");

        // SAFETY: This test holds a global mutex to serialize environment mutation.
        unsafe {
            env::remove_var(ENV_VAR);
        }

        let path = cache_path::<Vec<u64>>();
        let expected_root = default_cache_base().join(".pbt");
        assert!(
            path.starts_with(&expected_root),
            "{path:?} should live under {expected_root:?}"
        );
        assert!(
            path.starts_with(default_cache_root()),
            "{path:?} should live under {:?}",
            default_cache_root()
        );
    }
}
