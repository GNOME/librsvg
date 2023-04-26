#!/bin/sh

set -eux -o pipefail

clang_version=$(clang --version | head -n 1 | cut -d' ' -f 3 | cut -d'.' -f 1)
clang_libraries_path="/usr/lib64/clang/$clang_version/lib/linux"
clang_profile_lib="clang_rt.profile-x86_64"

if [ ! -d $clang_libraries_path ]
then
    echo "Expected clang libraries (for $clang_profile_lib) to be in $clang_libraries_path"
    echo "but that directory does not exist.  Please adjust the build-with-coverage.sh script."
    exit 1
fi

# Mixed gcc and Rust/LLVM coverage for the C API tests:
# https://searchfox.org/mozilla-central/source/browser/config/mozconfigs/linux64/code-coverage#15
export CC="clang"
export CFLAGS="-coverage -ftest-coverage -fprofile-arcs"
# RUSTFLAGS: "-Cinstrument-coverage"
export RUSTDOCFLAGS="-Cinstrument-coverage"
export LLVM_PROFILE_FILE="$(pwd)/coverage-profiles/coverage-%p-%m.profraw"
export LDFLAGS="--coverage -L$clang_libraries_path"
export LIBS="-l$clang_profile_lib"
export RUSTC_BOOTSTRAP="1"   # hack to make -Zprofile work on the non-nightly compiler
export CARGO_INCREMENTAL="0" # -Zprofile (gcov) doesn't like incremental compilation
export RUSTFLAGS="-Zprofile -Ccodegen-units=1 -Clink-dead-code -Coverflow-checks=off"

mkdir -p _build
cd _build
../autogen.sh --disable-gtk-doc --disable-vala --enable-debug
make
make -k check
