#!/usr/bin/env bash

shopt -s nullglob
set -euxo pipefail

nix flake check || nix flake check --offline

export CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER="valgrind --leak-check=full --show-leak-kinds=all --errors-for-leak-kinds=definite,indirect --error-exitcode=1 --trace-children=yes"
cargo test --workspace --target x86_64-unknown-linux-gnu
unset CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUNNER

cargo miri test # process-spawning tests are ignored under Miri in the Rust test suite

nix flake update || : # not added to git; this is to keep the *next* commit up to date without invalidating the above
cargo update || :
