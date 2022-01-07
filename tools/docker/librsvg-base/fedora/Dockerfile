FROM fedora
LABEL MAINTAINER=librsvg
RUN dnf update -y && dnf install -y gcc rust rust-std-static cargo make \
automake autoconf libtool gi-docgen git redhat-rpm-config \
gdk-pixbuf2-devel gobject-introspection-devel \
libxml2-devel cairo-devel cairo-gobject-devel pango-devel
