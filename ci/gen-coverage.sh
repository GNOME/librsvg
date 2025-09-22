#!/bin/sh

set -eu

mkdir -p public

call_grcov() {
    output_type=$1
    output_path=$2

    # Explanation of the options below:
    # grcov coverage-profiles _build               - paths where to find .rawprof (llvm) and .gcda (gcc) files, respectively
    #       --binary-path ./_build/target/debug/   - where the Rust test binaries are located
    #       --source-dir .                         - toplevel source directory
    #       --branch                               - compute branch coverage if possible
    #       --ignore '**/build/markup5ever*'       - ignore generated code from dependencies
    #       --ignore '**/build/cssparser*'         - ignore generated code from dependencies
    #       --ignore 'cargo_cache/*'               - ignore code from dependencies
    #       --ignore '_build/*'                    - ignore generated code
    #       --ignore 'rsvg-bench/*'                - ignore benchmarks; they are not useful for the test coverage report
    #       --excl-line 'unreachable!'             - ignore lines with the unreachable!() macro
    #       --output-type $output_type
    #       --output-path $output_path

    grcov coverage-profiles _build               \
          --binary-path ./_build/target/debug/   \
          --source-dir .                         \
          --branch                               \
          --ignore '**/build/markup5ever*'       \
          --ignore '**/build/cssparser*'         \
          --ignore 'cargo_cache/*'               \
          --ignore '_build/*'                    \
          --ignore 'rsvg-bench/*'                \
          --excl-line 'unreachable!'             \
          --output-type $output_type             \
          --output-path $output_path
}

call_grcov html public/coverage

# Disable the cobertura report for now; it is only used for showing coverage
# in the diff view of merge requests.
#
# After switching to gcov-based instrumentation (-Zprofile in .gitlab-ci.yml), this
# coverage.xml is almost 500 MB and causes gitlab's redis to OOM.
#
# call_grcov cobertura coverage.xml

# Print "Coverage: 42.42" so .gitlab-ci.yml will pick it up with a regex
#
# We scrape this from the HTML report, not the JSON summary, because coverage.json
# uses no decimal places, just something like "42%".

grep -Eo 'abbr title.* %' public/coverage/index.html | head -n 1 | grep -Eo '[0-9.]+ %' | grep -Eo '[0-9.]+' | awk '{ print "Coverage:", $1 }'
