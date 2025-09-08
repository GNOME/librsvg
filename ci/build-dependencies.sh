#!/bin/bash
# The following is to disable "info" warnings for the unquoted instandes of $MESON_FLAGS
# in the meson invocations below.  We want the shell to actually split $MESON_FLAGS by spaces.
# shellcheck disable=SC2086
#
# IMPORTANT: See
# https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/ci.html#container-image-version

set -o errexit -o pipefail -o noclobber -o nounset

# See the versions here:
# https://gitlab.gnome.org/GNOME/gnome-build-meta/-/tree/gnome-47/elements/sdk (or later versions)
# https://gitlab.com/freedesktop-sdk/freedesktop-sdk/-/tree/master/elements/components

FREETYPE2_TAG="VER-2-13-3"
FONTCONFIG_TAG="2.15.0"
CAIRO_TAG="1.18.2"
HARFBUZZ_TAG="9.0.0"
PANGO_TAG="1.54.0"
LIBXML2_TAG="v2.13.3"
GDK_PIXBUF_TAG="2.42.12"

PARSED=$(getopt --options '' --longoptions 'prefix:,meson-flags:' --name "$0" -- "$@")
eval set -- "$PARSED"
unset PARSED

PREFIX=
MESON_FLAGS=

while true; do
    case "$1" in
        '--prefix')
            PREFIX=$2
            shift 2
            ;;

        '--meson-flags')
            MESON_FLAGS=$2
            shift 2
            ;;

        '--')
            shift
            break
            ;;

        *)
            echo "Programming error"
            exit 3
            ;;
    esac
done

if [ -z "$PREFIX" ]; then
    echo "please specify a --prefix"
    exit 1
fi

# The following assumes that $PREFIX has been set
source ci/setup-dependencies-env.sh

cd ..
git clone --depth 1 --branch $FREETYPE2_TAG https://gitlab.freedesktop.org/freetype/freetype
cd freetype
meson setup _build --prefix "$PREFIX" -Dharfbuzz=disabled $MESON_FLAGS
meson compile -C _build
meson install -C _build

cd ..
git clone --depth 1 --branch $FONTCONFIG_TAG https://gitlab.freedesktop.org/fontconfig/fontconfig
cd fontconfig
meson setup _build --prefix "$PREFIX" $MESON_FLAGS
meson compile -C _build
meson install -C _build

cd ..
git clone --depth 1 --branch $CAIRO_TAG https://gitlab.freedesktop.org/cairo/cairo
cd cairo
meson setup _build --prefix "$PREFIX" $MESON_FLAGS
meson compile -C _build
meson install -C _build

cd ..
git clone --depth 1 --branch $HARFBUZZ_TAG https://github.com/harfbuzz/harfbuzz
cd harfbuzz
meson setup _build --prefix "$PREFIX" $MESON_FLAGS
meson compile -C _build
meson install -C _build

cd ..
git clone --depth 1 --branch $PANGO_TAG https://gitlab.gnome.org/GNOME/pango
cd pango
meson setup _build --prefix "$PREFIX" $MESON_FLAGS
meson compile -C _build
meson install -C _build

cd ..
git clone --depth 1 --branch $LIBXML2_TAG https://gitlab.gnome.org/GNOME/libxml2
cd libxml2
mkdir _build
cd _build
../autogen.sh --prefix "$PREFIX" --libdir "$PREFIX"/lib64 --without-python
make
make install

cd ..
git clone --depth 1 --branch $GDK_PIXBUF_TAG https://gitlab.gnome.org/GNOME/gdk-pixbuf
cd gdk-pixbuf
meson setup _build --prefix "$PREFIX" -Dman=false $MESON_FLAGS
meson compile -C _build
meson install -C _build
