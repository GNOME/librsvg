#!/bin/sh -e

BUILDDIR="../builddir"

set -x

cd "$(dirname "$0")"

if [ -d "../builddir" ]; then
    meson setup --reconfigure "$BUILDDIR" ../
else
    meson setup "$BUILDDIR" ../
fi

meson compile -C ../builddir

./gir/generator.py --no-fmt --gir-files-directories "$BUILDDIR/rsvg" gir-files/ $@
cargo fmt --all
