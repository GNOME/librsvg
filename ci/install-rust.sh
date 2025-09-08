#!/bin/bash
#
# IMPORTANT: See
# https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/ci.html#container-image-version

set -o errexit -o pipefail -o noclobber -o nounset

source ./ci/env.sh

export CARGO_HOME='/usr/local/cargo'

PARSED=$(getopt --options '' --longoptions 'rustup-version:,stable:,minimum:,nightly:,arch:' --name "$0" -- "$@")
eval set -- "$PARSED"
unset PARSED

RUSTUP_VERSION=
STABLE=
MINIMUM=
NIGHTLY=
ARCH=

while true; do
    case "$1" in
        '--rustup-version')
            RUSTUP_VERSION=$2
            shift 2
            ;;

        '--stable')
            STABLE=$2
            shift 2
            ;;

        '--minimum')
            MINIMUM=$2
            shift 2
            ;;

        '--nightly')
            NIGHTLY=$2
            shift 2
            ;;

        '--arch')
            ARCH=$2
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

if [ -z "$RUSTUP_VERSION" ]; then
    echo "missing --rustup-version argument"
    exit 1
fi

if [ -z "$STABLE" ]; then
    echo "missing --stable argument, please pass the stable version of rustc you want"
    exit 1
fi
    
if [ -z "$ARCH" ]; then
    echo "missing --arch argument, please pass an architecture triple like x86_64-unknown-linux-gnu"
    exit 1
fi

RUSTUP_URL="https://static.rust-lang.org/rustup/archive/$RUSTUP_VERSION/$ARCH/rustup-init"
wget "$RUSTUP_URL"

chmod +x rustup-init
./rustup-init -y --no-modify-path --profile minimal --default-toolchain "$STABLE"
rm rustup-init
chmod -R a+w "$RUSTUP_HOME" "$CARGO_HOME"

if [ -n "$MINIMUM" ]; then
    rustup toolchain install "$MINIMUM"
fi

if [ -n "$NIGHTLY" ]; then
    rustup toolchain install "$NIGHTLY"
fi
