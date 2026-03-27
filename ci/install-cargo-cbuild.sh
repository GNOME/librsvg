#!/bin/bash

# IMPORTANT: See
# https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/ci.html#container-image-version

source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

cargo install --force --locked cargo-c --version 0.10.10
