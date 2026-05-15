#!/usr/bin/env bash

set -euo pipefail

host_target="$(rustc -vV | sed -n 's/^host: //p')"
runner_env="CARGO_TARGET_${host_target^^}_RUNNER"
runner_env="${runner_env//-/_}"
valgrind_runner='valgrind --quiet --error-exitcode=1 --leak-check=full --show-leak-kinds=definite,indirect --errors-for-leak-kinds=definite,indirect'
env "$runner_env=$valgrind_runner" \
    cargo test --all-features --all-targets --quiet --workspace

cargo clippy --all-features --all-targets --quiet --workspace

nix flake check --quiet --no-warn-dirty 2> >(
    grep -v -E \
        -e '^warning: The check omitted these incompatible systems: ' \
        -e "^Use '--all-systems' to check all\\.$" >&2
)

MIRIFLAGS='-Zmiri-disable-isolation' cargo miri test --all-features --all-targets --quiet --workspace # TODO: seems not to be available via Nix/Crane
