//! Filesystem persistence for minimized witnesses.

use {
    crate::{
        Pbt,
        hash::random_state,
        reflection::{Parts, erased_vec_ops_of},
    },
    core::{any::TypeId, fmt::Write as _},
    std::{
        env::{self, current_dir},
        fs::{File, OpenOptions, create_dir_all},
        io::{BufRead as _, BufReader, Write as _},
        path::PathBuf,
    },
};

/// Find the Cargo workspace root by walking upward to `Cargo.lock`.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
fn workspace_root() -> PathBuf {
    let cwd = current_dir().expect("INTERNAL ERROR (`pbt`): couldn't read the current directory");
    cwd.ancestors()
        .find(|dir| dir.join("Cargo.lock").exists())
        .expect("INTERNAL ERROR (`pbt`): couldn't find the Cargo workspace root")
        .to_owned()
}

/// The directory holding persisted witnesses.
#[inline]
fn dir() -> PathBuf {
    if let Ok(cache_dir) = env::var("PBT_CACHE_DIR")
        && !cache_dir.is_empty()
    {
        PathBuf::from(cache_dir)
    } else {
        workspace_root().join(".pbt")
    }
}

/// The path to the JSONL file holding persisted witnesses of this type.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn jsonl_path<T>() -> PathBuf
where
    T: 'static,
{
    let ty = TypeId::of::<T>();
    let erased_vec_ops = erased_vec_ops_of(ty);

    let type_name = (erased_vec_ops.name)();
    let mut jsonl_filename = String::new();
    let () = write!(
        jsonl_filename,
        "{:016X}",
        random_state().hash_one(type_name),
    )
    .expect("INTERNAL ERROR (`pbt`): couldn't write to a `String`");
    let () = jsonl_filename.push('-');
    let () = jsonl_filename.extend(
        type_name
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }),
    );
    let () = jsonl_filename.push_str(".jsonl");

    dir().join(jsonl_filename)
}

/// Replay any witnesses persisted for this type.
///
/// The corpus is read into memory before this returns, so its shared filesystem
/// lock is not held while the caller evaluates properties over the witnesses.
///
/// # Panics
///
/// If persisted witnesses could not be found because of unexpected I/O errors,
/// not because the directory does not exist
/// (in which case this will safely return an empty vector).
#[inline]
#[must_use]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
#[expect(
    clippy::absolute_paths,
    reason = "`core::io::ErrorKind` is unstable; filesystem errors use stable `std::io`."
)]
pub fn replay<T>() -> Vec<T>
where
    T: Pbt,
{
    if let Ok(no_replay) = env::var("PBT_NO_REPLAY")
        && !no_replay.is_empty()
        && no_replay != "0"
    {
        Vec::new()
    } else {
        match File::open(jsonl_path::<T>()) {
            Ok(file) => {
                let () = file
                    .lock_shared()
                    .expect("INTERNAL ERROR (`pbt`): couldn't lock persisted witnesses");
                BufReader::new(file)
                    .lines()
                    .map(|line_result| {
                        let line = line_result
                            .expect("INTERNAL ERROR (`pbt`): couldn't read persisted witnesses");
                        let json = serde_json::from_str(&line)
                            .expect("INTERNAL ERROR (`pbt`): couldn't parse persisted JSONL");
                        Parts::deserialize(&json).expect(
                            "INTERNAL ERROR (`pbt`): couldn't deserialize a persisted witness",
                        )
                    })
                    .collect()
            }
            Err(error) => {
                assert_eq!(
                    error.kind(),
                    std::io::ErrorKind::NotFound,
                    "INTERNAL ERROR (`pbt`): couldn't read persisted witnesses",
                );
                Vec::new()
            }
        }
    }
}

/// Persist a witness of this type to its JSONL corpus.
///
/// The duplicate check and append form one filesystem-locked transaction.
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn witness<T>(t: &T)
where
    T: Pbt,
{
    let json = t.clone().deconstruct().serialize();
    let mut record =
        serde_json::to_vec(&json).expect("INTERNAL ERROR (`pbt`): couldn't serialize a witness");
    let () = record.push(b'\n');
    let path = jsonl_path::<T>();

    let () = create_dir_all(dir())
        .expect("INTERNAL ERROR (`pbt`): couldn't create the persistence directory");
    let mut file = OpenOptions::new()
        .read(true)
        .append(true)
        .create(true)
        .open(path)
        .expect("INTERNAL ERROR (`pbt`): couldn't persist a witness to the filesystem");
    let () = file
        .lock()
        .expect("INTERNAL ERROR (`pbt`): couldn't lock persisted witnesses");

    for line_result in BufReader::new(&file).lines() {
        let line = line_result.expect("INTERNAL ERROR (`pbt`): couldn't read persisted witnesses");
        let persisted: serde_json::Value = serde_json::from_str(&line)
            .expect("INTERNAL ERROR (`pbt`): couldn't parse persisted JSONL");
        if persisted == json {
            return;
        }
    }

    let () = file
        .write_all(&record)
        .expect("INTERNAL ERROR (`pbt`): couldn't persist a witness");
}

#[cfg(test)]
mod tests {
    #![expect(
        clippy::expect_used,
        reason = "Subprocess and filesystem failures should fail tests loudly."
    )]

    use {
        super::*,
        crate::reflection::register_globally,
        core::time::Duration,
        std::{
            fs::{self, File},
            io::ErrorKind,
            process::{self, Command, Stdio},
            thread,
        },
    };

    /// Environment variable selecting the witness written by a subprocess.
    const CHILD_WITNESS: &str = "PBT_TEST_CHILD_WITNESS";

    /// Environment variable naming the file that releases all waiting subprocesses.
    const START_FILE: &str = "PBT_TEST_START_FILE";

    /// Number of elements used to make concurrent writes long enough to overlap.
    const WITNESS_ELEMENTS: usize = 0x0400;

    /// Write one large witness when invoked as a child of the concurrency test.
    #[test]
    fn concurrent_witness_child() {
        let Ok(element_text) = env::var(CHILD_WITNESS) else {
            return;
        };
        let start_file =
            PathBuf::from(env::var(START_FILE).expect("parent should provide the start file"));
        while !start_file.exists() {
            thread::sleep(Duration::from_millis(1));
        }

        let element = element_text
            .parse::<usize>()
            .expect("parent should provide a `usize` witness");
        let () = register_globally::<[usize; WITNESS_ELEMENTS]>();
        witness(&[element; WITNESS_ELEMENTS]);
    }

    /// Concurrent processes append complete, de-duplicated JSONL records.
    #[test]
    #[cfg_attr(
        miri,
        ignore = "unsupported operation: can't call foreign function `posix_spawnattr_init` on OS `linux`"
    )]
    fn concurrent_witnesses_remain_valid_jsonl() {
        const DISTINCT_WITNESSES: usize = 4;
        const PROCESSES: usize = 16;

        let root = env::temp_dir().join(format!("pbt-concurrent-witnesses-{}", process::id()));
        match fs::remove_dir_all(&root) {
            Ok(()) => {}
            Err(error) => assert_eq!(
                error.kind(),
                ErrorKind::NotFound,
                "couldn't remove a stale persistence test directory",
            ),
        }
        let () = fs::create_dir_all(&root).expect("couldn't create the persistence test directory");
        let cache_dir = root.join("cache");
        let start_file = root.join("start");
        let test_binary = env::current_exe().expect("couldn't locate the test binary");

        let mut children = (0..DISTINCT_WITNESSES)
            .cycle()
            .take(PROCESSES)
            .map(|element| {
                Command::new(&test_binary)
                    .arg("--exact")
                    .arg("persist::tests::concurrent_witness_child")
                    .arg("--quiet")
                    .stdout(Stdio::null())
                    .env("PBT_CACHE_DIR", &cache_dir)
                    .env(CHILD_WITNESS, element.to_string())
                    .env(START_FILE, &start_file)
                    .spawn()
                    .expect("couldn't spawn a persistence test subprocess")
            })
            .collect::<Vec<_>>();

        let _start = File::create(&start_file).expect("couldn't release test subprocesses");
        for child in &mut children {
            assert!(
                child
                    .wait()
                    .expect("couldn't wait for a test subprocess")
                    .success(),
                "persistence test subprocess failed",
            );
        }

        let mut corpus_entries =
            fs::read_dir(&cache_dir).expect("couldn't read the persistence test directory");
        let corpus_path = corpus_entries
            .next()
            .expect("persistence test did not create a corpus")
            .expect("couldn't read the persisted corpus entry")
            .path();
        assert!(
            corpus_entries.next().is_none(),
            "persistence test unexpectedly created multiple corpora",
        );

        let mut actual = BufReader::new(
            File::open(corpus_path).expect("couldn't open the persisted test corpus"),
        )
        .lines()
        .map(|line| line.expect("couldn't read the persisted test corpus"))
        .collect::<Vec<_>>();
        for line in &actual {
            let _: serde_json::Value =
                serde_json::from_str(line).expect("concurrent persistence corrupted JSONL");
        }
        actual.sort_unstable();

        let () = register_globally::<[usize; WITNESS_ELEMENTS]>();
        let mut expected = (0..DISTINCT_WITNESSES)
            .map(|element| {
                serde_json::to_string(&[element; WITNESS_ELEMENTS].deconstruct().serialize())
                    .expect("couldn't serialize an expected test witness")
            })
            .collect::<Vec<_>>();
        expected.sort_unstable();
        assert_eq!(actual, expected);

        let () = fs::remove_dir_all(root).expect("couldn't remove the persistence test directory");
    }
}
