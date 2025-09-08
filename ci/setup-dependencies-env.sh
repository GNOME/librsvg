#!/bin/bash
#
# IMPORTANT: See
# https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/ci.html#container-image-version

if [ -z "$PREFIX" ]; then
    echo "Using default prefix /usr/local/librsvg for dependencies."
    echo "If this is not what you want, set the PREFIX variable"
    echo "before sourcing this script."
    PREFIX=/usr/local/librsvg
fi

export PATH=$PREFIX/bin:$PATH
export LD_LIBRARY_PATH=$PREFIX/lib64
export PKG_CONFIG_PATH=$PREFIX/lib64/pkgconfig
export XDG_DATA_DIRS=${PREFIX}/share:/usr/share
export ACLOCAL_PATH=${PREFIX}/share/aclocal
