#!/bin/bash
#
# IMPORTANT: See
# https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/ci.html#container-image-version

source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

# Coverage tools
cargo install grcov
rustup component add llvm-tools-preview
