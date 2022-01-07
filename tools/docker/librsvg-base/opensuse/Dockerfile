FROM opensuse/tumbleweed
LABEL MAINTAINER=librsvg
RUN zypper refresh && zypper install -y gcc rust rust-std cargo make \
automake autoconf libtool python3-gi-docgen python38-docutils git \
gdk-pixbuf-devel gobject-introspection-devel \
libxml2-devel cairo-devel pango-devel
