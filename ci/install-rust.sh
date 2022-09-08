source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

RUSTUP_VERSION=1.25.1
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
