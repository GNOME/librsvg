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
    mingw-w64-$MSYS2_ARCH-meson \
    mingw-w64-$MSYS2_ARCH-cargo-c \
    mingw-w64-$MSYS2_ARCH-gi-docgen \
    mingw-w64-$MSYS2_ARCH-gobject-introspection \
    mingw-w64-$MSYS2_ARCH-gdk-pixbuf2 \
    mingw-w64-$MSYS2_ARCH-harfbuzz \
    mingw-w64-$MSYS2_ARCH-fontconfig \
    mingw-w64-$MSYS2_ARCH-fribidi \
    mingw-w64-$MSYS2_ARCH-libthai \
    mingw-w64-$MSYS2_ARCH-cairo \
    mingw-w64-$MSYS2_ARCH-pango \
    mingw-w64-$MSYS2_ARCH-python-docutils \
    mingw-w64-$MSYS2_ARCH-libxml2 \
    mingw-w64-$MSYS2_ARCH-toolchain \
    mingw-w64-$MSYS2_ARCH-rust \
    mingw-w64-$MSYS2_ARCH-cantarell-fonts

meson setup _build -Dauto_features=disabled
meson compile -C _build
export RUST_BACKTRACE=1
export TESTS_OUTPUT_DIR=$(pwd)/tests/output
# meson test -C _build --print-errorlogs
