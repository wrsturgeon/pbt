#!/usr/bin/env bash

shopt -s nullglob
set -euxo pipefail

nix flake check --offline

cargo miri test # seems not to be available via Nix/Crane; TODO

nix flake update || : # not added to git; this is to keep the *next* commit up to date without invalidating the above
cargo update || :
