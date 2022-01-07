FROM debian:testing
LABEL MAINTAINER=librsvg
RUN apt-get update && apt-get upgrade -y && apt-get install -y gcc make rustc cargo \
automake autoconf libtool gi-docgen git \
libgdk-pixbuf2.0-dev libgirepository1.0-dev \
libxml2-dev libcairo2-dev libpango1.0-dev
