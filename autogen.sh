#!/bin/sh
# Run this to generate all the initial makefiles, etc.

srcdir=`dirname $0`
test -z "$srcdir" && srcdir=.

PKG_NAME="librsvg"

(test -f $srcdir/configure.in \
  && test -f $srcdir/README \
  && test -f $srcdir/rsvg.h) || {
    echo -n "**Error**: Directory "\`$srcdir\'" does not look like the"
    echo " top-level $PKG_NAME directory"
    exit 1
}

ACLOCAL_FLAGS="-I hack-macros $ACLOCAL_FLAGS"
aclocal $ACLOCAL_FLAGS

which gnome-autogen.sh || {
    echo "You need to install gnome-common from the GNOME CVS"
    exit 1
}
USE_GNOME2_MACROS=1 . gnome-autogen.sh
