source ./ci/env.sh

set -eu
export CARGO_HOME='/usr/local/cargo'

# Coverage tools
cargo install grcov
rustup component add llvm-tools-preview
