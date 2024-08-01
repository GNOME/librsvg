#!/bin/sh
#
# Generates the Rust documentation in the following directories:
#   public/internals - internals documentation, for librsvg development
#   public/doc       - public API documentation

set -eu

cargo doc --workspace --document-private-items --no-deps
mkdir -p public/internals
mv target/doc/* public/internals

cargo doc --no-deps --package librsvg --package 'librsvg-rebind*'
mkdir -p public/doc
cp -r target/doc/* public/doc
