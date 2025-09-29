source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

# Coverage tools
cargo install --locked grcov --version 0.8.19
rustup component add llvm-tools-preview
