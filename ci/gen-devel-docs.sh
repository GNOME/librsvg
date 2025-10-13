#!/bin/sh

set -eu

mkdir -p public/devel-docs
sphinx-build devel-docs public/devel-docs

