#!/bin/bash

set -eux -o pipefail

clang_version=$(clang --version | head -n 1 | cut -d' ' -f 3 | cut -d'.' -f 1)
clang_libraries_path="/usr/lib64/clang/$clang_version/lib/linux"
clang_profile_lib="clang_rt.profile-x86_64"

if [ ! -d "$clang_libraries_path" ]
then
    echo "Expected clang libraries (for $clang_profile_lib) to be in $clang_libraries_path"
    echo "but that directory does not exist.  Please adjust the build-with-coverage.sh script."
    exit 1
fi

# Mixed gcc and Rust/LLVM coverage for the C API tests:
# https://searchfox.org/mozilla-central/source/browser/config/mozconfigs/linux64/code-coverage#15
export CC="clang"
export RUSTDOCFLAGS="-C instrument-coverage"
LLVM_PROFILE_FILE="$(pwd)/coverage-profiles/coverage-%p-%m.profraw"
export LLVM_PROFILE_FILE
export RUSTC_BOOTSTRAP="1"   # hack to make unstable options work on the non-nightly compiler
export RUSTFLAGS="-C instrument-coverage -Z coverage-options=condition -Ccodegen-units=1 -Clink-dead-code -Coverflow-checks=off"

# meson setup _build -Db_coverage=true -Dauto_features=disabled -Dpixbuf{,-loader}=enabled --buildtype=debugoptimized
# meson compile -C _build
# meson test -C _build --maxfail 0 --print-errorlogs

cargo test -- --include-ignored
