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
CARGO=$(where cargo)
export CARGO

RUSTC=$(where rustc)
export RUSTC

meson setup _build -Dauto_features=disabled -Dpixbuf{,-loader}=enabled
meson compile -C _build

export RUST_BACKTRACE=1
TESTS_OUTPUT_DIR=$(pwd)/tests/output
export TESTS_OUTPUT_DIR
# meson test -C _build --print-errorlogs
