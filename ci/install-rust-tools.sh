#!/bin/bash

# IMPORTANT: See
# https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/ci.html#container-image-version

source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

rustup component add clippy
rustup component add rustfmt
cargo install --force --locked cargo-c --version 0.10.10
cargo install --version ^1.0 gitlab_clippy
cargo install --force --locked cargo-deny
# cargo install --force cargo-outdated
