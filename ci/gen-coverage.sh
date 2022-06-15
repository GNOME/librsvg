#!/bin/sh

set -eu

mkdir -p public

call_grcov() {
    output_type=$1
    output_path=$2

    grcov coverage-profiles _build               \
          --binary-path ./_build/target/debug/   \
          --source-dir .                         \
          --prefix-dir ../../                    \
          --branch                               \
          --ignore build.rs                      \
          --ignore '**/build/markup5ever*'       \
          --ignore '**/build/cssparser*'         \
          --output-type $output_type             \
          --output-path $output_path
}

call_grcov cobertura coverage.xml
call_grcov html public/coverage

# Print "Coverage: 42.42" so .gitlab-ci.yml will pick it up with a regex
grep -Eo 'line-rate="[^"]+"' coverage.xml | head -n 1 | grep -Eo '[0-9.]+' | awk '{ print "Coverage:", $1 * 100 }'
