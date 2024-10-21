#!/bin/sh

set -eu

mkdir -p public/devel-docs
sphinx-build --fail-on-warning --keep-going devel-docs public/devel-docs

