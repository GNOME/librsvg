FROM fedora
MAINTAINER librsvg
RUN dnf update && dnf install -y gcc rust rust-std-static cargo make \
automake autoconf libtool gtk-doc git redhat-rpm-config \
gdk-pixbuf2-devel gobject-introspection-devel \
libxml2-devel cairo-devel cairo-gobject-devel pango-devel
