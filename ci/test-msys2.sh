#!/bin/bash

set -e

if [[ "$MSYSTEM" == "MINGW32" ]]; then
    export MSYS2_ARCH="i686"
else
    export MSYS2_ARCH="x86_64"
fi

pacman --noconfirm -Suy

pacman --noconfirm -S --needed \
    base-devel \
    pactoys

pacboy --noconfirm -S --needed \
    meson:p \
    cargo-c:p \
    gi-docgen:p \
    gobject-introspection:p \
    gdk-pixbuf2:p \
    harfbuzz:p \
    fontconfig:p \
    fribidi:p \
    libthai:p \
    cairo:p \
    pango:p \
    python-docutils:p \
    libxml2:p \
    toolchain:p \
    rust:p \
    cantarell-fonts:p

# https://github.com/rust-lang/cargo/issues/10885
export CARGO=$(where cargo)
export RUSTC=$(where rustc)
meson setup _build -Dauto_features=disabled
meson compile -C _build
export RUST_BACKTRACE=1
export TESTS_OUTPUT_DIR=$(pwd)/tests/output
# meson test -C _build --print-errorlogs
