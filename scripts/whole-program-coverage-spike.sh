#!/usr/bin/env bash

set -euo pipefail

if [[ "$(uname -s)" != 'Linux' ]]; then
    echo 'error: the whole-program coverage instrumentation spike supports Linux only' >&2
    exit 1
fi

if [[ -n "${RUSTFLAGS-}" || -n "${CARGO_ENCODED_RUSTFLAGS-}" ]]; then
    echo 'error: unset RUSTFLAGS and CARGO_ENCODED_RUSTFLAGS before running this spike' >&2
    exit 1
fi

host="$(rustc --print host-tuple)"

LLVM_PROFILE_FILE=/dev/null \
PBT_COVERAGE_SPIKE=1 \
RUSTFLAGS='-Ctarget-cpu=native -Cinstrument-coverage' \
    cargo test \
        --target "$host" \
        -p pbt-tests \
        --lib \
        coverage_spike::whole_program_coverage_spike \
        -- \
        --exact \
        --nocapture \
        --test-threads=1
