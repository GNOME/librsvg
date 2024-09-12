source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

rustup component add clippy
rustup component add rustfmt
cargo install --force --locked cargo-c
cargo install --version ^1.0 gitlab_clippy
cargo install --force cargo-deny
# cargo install --force cargo-outdated
