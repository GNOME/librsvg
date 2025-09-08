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

    grcov coverage-profiles                      \
          --binary-path ./target/debug/          \
          --source-dir .                         \
          --branch                               \
          --ignore 'cargo_cache/*'               \
          --ignore 'target/*'                    \
          --excl-line 'unreachable!'             \
          --output-type "$output_type"           \
          --output-path "$output_path"
}

call_grcov html public/coverage

# Generate the cobertura XML format for GitLab's line-by-line coverage report in MR diffs.
#
# However, guard it for not being over 10 MB in size; we had a case before where it was over
# 500 MB and that OOM'd gitlab's redis.

call_grcov cobertura coverage.xml
size=$(wc -c < coverage.xml)
if [ "$size" -ge 10485760 ]
then
    rm coverage.xml
    echo "coverage.xml is over 10 MB, removing it so it will not be used"
fi

# Print "Coverage: 42.42" so .gitlab-ci.yml will pick it up with a regex
#
# We scrape this from the HTML report, not the JSON summary, because coverage.json
# uses no decimal places, just something like "42%".

grep -Eo 'abbr title.* %' public/coverage/index.html | head -n 1 | grep -Eo '[0-9.]+ %' | grep -Eo '[0-9.]+' | awk '{ print "Coverage:", $1 }'
