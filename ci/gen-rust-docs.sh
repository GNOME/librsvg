#!/bin/sh
#
# Generates the Rust documentation in the following directories:
#   public/internals - internals documentation, for librsvg development
#   public/doc       - public API documentation

set -eu

mkdir -p public/internals
cargo doc --workspace --document-private-items --no-deps
cp -r target/doc/* public/internals

mkdir -p public/doc
cargo doc --no-deps --package librsvg
cp -r target/doc/* public/doc
