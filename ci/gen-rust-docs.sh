#!/bin/sh
#
# Generates the Rust documentation in the following directories:
#   public/internals - internals documentation, for librsvg development
#   public/doc       - public API documentation

set -eu

# turn warnings into errors
export RUSTDOCFLAGS='-D warnings'

cargo doc --workspace --document-private-items --no-deps
# cargo doc --document-private-items --no-deps --package librsvg
mkdir -p public/internals
mv target/doc/* public/internals

cargo doc --no-deps --package librsvg --package 'librsvg-rebind*'
# cargo doc --no-deps --package librsvg
mkdir -p public/doc
cp -r target/doc/* public/doc
