#!/usr/bin/env bash

shopt -s nullglob
set -euxo pipefail

nix flake check
