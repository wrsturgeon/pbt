#!/usr/bin/env bash

set -euo pipefail

echo
echo 'Running all tests with `valgrind`...'
host_target="$(rustc -vV | sed -n 's/^host: //p')"
runner_env="CARGO_TARGET_${host_target^^}_RUNNER"
runner_env="${runner_env//-/_}"
valgrind_runner='valgrind --quiet --error-exitcode=1 --leak-check=full --show-leak-kinds=definite,indirect --errors-for-leak-kinds=definite,indirect'
env "$runner_env=$valgrind_runner" \
    cargo test --all-features --all-targets --quiet --workspace

echo 'Linting with `cargo clippy`...'
cargo clippy --all-features --all-targets --quiet --workspace

echo
echo 'Comprehensive checks with `nix flake check`...'
nix flake check --quiet --no-warn-dirty 2> >(
    grep -v -E \
        -e '^warning: The check omitted these incompatible systems: ' \
        -e "^Use '--all-systems' to check all\\.$" >&2
)

echo
echo 'Running all tests with `miri`...'
MIRIFLAGS='-Zmiri-disable-isolation' cargo miri test --all-features --all-targets --quiet --workspace # TODO: seems not to be available via Nix/Crane
