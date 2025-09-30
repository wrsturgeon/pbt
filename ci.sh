#!/usr/bin/env bash

set -euxo pipefail
shopt -s nullglob

nix flake check --all-systems

direnv allow
cargo miri test --no-default-features
cargo miri test --all-features
cargo tarpaulin --fail-under 90
