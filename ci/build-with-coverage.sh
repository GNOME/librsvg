#!/bin/sh

set -eux -o pipefail

# Mixed gcc and Rust/LLVM coverage for the C API tests:
# https://searchfox.org/mozilla-central/source/browser/config/mozconfigs/linux64/code-coverage#15
export CC="clang"
export CFLAGS="-coverage -ftest-coverage -fprofile-arcs"
# RUSTFLAGS: "-Cinstrument-coverage"
export RUSTDOCFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="coverage-profiles/coverage-%p-%m.profraw"
export LDFLAGS="--coverage -L/usr/lib64/clang/14.0.6/lib/linux"
export LIBS="-lclang_rt.profile-x86_64"
export RUSTC_BOOTSTRAP="1"   # hack to make -Zprofile work on the non-nightly compiler
export CARGO_INCREMENTAL="0" # -Zprofile (gcov) doesn't like incremental compilation
export RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Clink-dead-code -Coverflow-checks=off"

mkdir -p _build
cd _build
../autogen.sh --disable-gtk-doc --disable-vala --enable-debug
make
make -k check
