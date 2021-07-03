#!/bin/sh
# This script assumes that "cargo cbuild" has been run already.

set -e

TARGET=target/x86_64-unknown-linux-gnu/debug

# FIXME: cargo-c renames the .so to .so.x.y.z on installation
# LIBRARY=librsvg-2.so.2.51.3
LIBRARY=librsvg-2.so

if [ ! -f $TARGET/$LIBRARY ]
then
    echo "error: $LIBRARY does not exist"
    exit 1
fi

if (objdump -p $TARGET/$LIBRARY | grep '  SONAME               librsvg-2.so.2')
then
    true
else
    echo "error: wrong SONAME"
    exit 1
fi

if [ ! -f $TARGET/librsvg-2.0.pc ]
then
    echo "error: missing librsvg-2.0.pc"
    exit 1
fi

if grep "Name: librsvg" librsvg-2.0.pc
then
    true
else
    echo "error: wrong Name in librsvg-2.0.pc"
    exit 1
fi

if grep 'Cflags: -I${includedir}/librsvg-2.0' librsvg-2.0.pc
then
    true
else
    echo "error: wrong Cflags in librsvg-2.0.pc"
    exit 1
fi

