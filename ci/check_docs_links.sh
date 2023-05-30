#!/bin/sh

set -eu

mkdir -p public/devel-docs-check
sphinx-build -b linkcheck devel-docs public/devel-docs-check
