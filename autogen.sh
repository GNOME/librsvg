#!/bin/sh
# Run this to generate all the initial makefiles, etc.

#name of package
PKG_NAME="librsvg"

REQUIRED_AUTOMAKE_VERSION=1.7.1
REQUIRED_AUTOCONF_VERSION=2.54

USE_GNOME2_MACROS=1

srcdir=`dirname $0`
test -z "$srcdir" && srcdir=.

(test -f $srcdir/configure.in \
  && test -d $srcdir/gdk-pixbuf-loader \
  && test -f $srcdir/gdk-pixbuf-loader/io-svg.c) || {
    echo -n "**Error**: Directory "\`$srcdir\'" does not look like the"
    echo " top-level librsvg directory"
    exit 1
}

ifs_save="$IFS"; IFS=":"
for dir in $PATH ; do
  IFS="$ifs_save"
  test -z "$dir" && dir=.
  if test -f $dir/gnome-autogen.sh ; then
    gnome_autogen="$dir/gnome-autogen.sh"
    gnome_datadir=`echo $dir | sed -e 's,/bin$,/share,'`
    break
  fi
done

if test -z "$gnome_autogen" ; then
  echo "You need to install the gnome-common module and make"
  echo "sure the gnome-autogen.sh script is in your \$PATH."
  exit 1
fi

GNOME_DATADIR="$gnome_datadir"
. $gnome_autogen
