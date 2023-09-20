#!/bin/bash

set -o errexit -o pipefail -o noclobber -o nounset

source ./ci/env.sh

export CARGO_HOME='/usr/local/cargo'

PARSED=$(getopt --options '' --longoptions 'rustup-version:,version:,arch:' --name "$0" -- "$@")
if [ $? -ne 0 ]; then
	echo 'Terminating...' >&2
	exit 1
fi

eval set -- "$PARSED"
unset PARSED

RUSTUP_VERSION=
VERSION=
ARCH=

while true; do
    case "$1" in
        '--rustup-version')
            RUSTUP_VERSION=$2
            shift 2
            ;;

        '--version')
            VERSION=$2
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

if [ -z "$VERSION"]; then
    echo "missing --version argument, please pass the version of rustc you want"
    exit 1
fi
    
if [ -z "$ARCH"]; then
    echo "missing --arch argument, please pass an architecture triple like x86_64-unknown-linux-gnu"
    exit 1
fi

RUSTUP_URL=https://static.rust-lang.org/rustup/archive/$RUSTUP_VERSION/$ARCH/rustup-init
wget $RUSTUP_URL

chmod +x rustup-init;
./rustup-init -y --no-modify-path --profile minimal --default-toolchain $VERSION;
rm rustup-init;
chmod -R a+w $RUSTUP_HOME $CARGO_HOME

rustup --version
cargo --version
rustc --version
