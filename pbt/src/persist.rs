//! Filesystem persistence for minimized witnesses.

use {
    crate::{
        Pbt,
        hash::random_state,
        reflection::{Parts, bucket_ops_of},
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
    let bucket_ops = bucket_ops_of(ty);

    let type_name = (bucket_ops.name)();
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
#[inline]
#[expect(
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
#[expect(
    clippy::absolute_paths,
    reason = "`core::io::ErrorKind` is unstable; filesystem errors use stable `std::io`."
)]
pub(crate) fn replay<T>() -> impl Iterator<Item = T>
where
    T: Pbt,
{
    if let Ok(no_replay) = env::var("PBT_NO_REPLAY")
        && !no_replay.is_empty()
        && no_replay != "0"
    {
        None
    } else {
        match File::open(jsonl_path::<T>()) {
            Ok(file) => Some(file),
            Err(error) => {
                assert_eq!(
                    error.kind(),
                    std::io::ErrorKind::NotFound,
                    "INTERNAL ERROR (`pbt`): couldn't read persisted witnesses",
                );
                None
            }
        }
    }
    .into_iter()
    .flat_map(|file| BufReader::new(file).lines())
    .map(|line_result| {
        let line = line_result.expect("INTERNAL ERROR (`pbt`): couldn't read persisted witnesses");
        let json = serde_json::from_str(&line)
            .expect("INTERNAL ERROR (`pbt`): couldn't parse persisted JSONL");
        Parts::deserialize(&json)
            .expect("INTERNAL ERROR (`pbt`): couldn't deserialize a persisted witness")
    })
}

/// Persist a witness of this type to its JSONL corpus.
#[inline]
#[expect(
    clippy::absolute_paths,
    clippy::expect_used,
    reason = "Internal invariants: violations should fail loudly."
)]
pub(crate) fn witness<T>(t: &T)
where
    T: Pbt,
{
    let json = t.clone().deconstruct().serialize();
    let path = jsonl_path::<T>();

    match File::open(&path) {
        Ok(file) => {
            for line_result in BufReader::new(file).lines() {
                let line =
                    line_result.expect("INTERNAL ERROR (`pbt`): couldn't read persisted witnesses");
                let persisted: serde_json::Value = serde_json::from_str(&line)
                    .expect("INTERNAL ERROR (`pbt`): couldn't parse persisted JSONL");
                if persisted == json {
                    return;
                }
            }
        }
        Err(error) => {
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::NotFound,
                "INTERNAL ERROR (`pbt`): couldn't read persisted witnesses",
            );
        }
    }

    let () = create_dir_all(dir())
        .expect("INTERNAL ERROR (`pbt`): couldn't create the persistence directory");
    let mut file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(path)
        .expect("INTERNAL ERROR (`pbt`): couldn't persist a witness to the filesystem");
    let () = serde_json::to_writer(&mut file, &json)
        .expect("INTERNAL ERROR (`pbt`): couldn't serialize a witness");
    let () = writeln!(file).expect("INTERNAL ERROR (`pbt`): couldn't persist a witness");
}
