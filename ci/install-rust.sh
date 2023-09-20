source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

if [ -z "$RUSTUP_VERSION" ]
then
    echo "RUSTUP_VERSION is not set, please set it"
    exit 1
fi

RUST_VERSION=$1
RUST_ARCH=$2

RUSTUP_URL=https://static.rust-lang.org/rustup/archive/$RUSTUP_VERSION/$RUST_ARCH/rustup-init
wget $RUSTUP_URL

chmod +x rustup-init;
./rustup-init -y --no-modify-path --profile minimal --default-toolchain $RUST_VERSION;
rm rustup-init;
chmod -R a+w $RUSTUP_HOME $CARGO_HOME

rustup --version
cargo --version
rustc --version
