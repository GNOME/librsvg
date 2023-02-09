# This script checks that the Minimum Supported Rust Version (MSRV) has the same value
# in several places throughout the source tree.

import re
import sys

PLACES_TO_CHECK = [
    ['configure.ac',                     r'MINIMUM_RUST_VER=(.*)'],
    ['Cargo.toml',                       r'rust-version\s*=\s*"(.*)"'],
    ['ci/container_builds.yml',          r'RUST_MINIMUM:\s*"(.*)"'],
    ['devel-docs/devel_environment.rst', r'rust (.*) or later'],
]

versions = []

for filename, regex in PLACES_TO_CHECK:
    r = re.compile(regex)

    with open(filename) as f:
        matched = False
        for idx, line in enumerate(f.readlines()):
            matches = r.search(line)
            if matches is not None:
                matched = True
                line_number = idx + 1
                versions.append([filename, line_number, matches.group(1), line])

        if not matched:
            raise Exception(f'file {filename} does not have a line that matches {regex}')

assert len(versions) > 0

all_the_same = True

for filename, line_number, version, line in versions[1:]:
    if version != versions[0][2]:
        all_the_same = False

if not all_the_same:
    print(f'Version numbers do not match in these lines, please fix them!\n', file=sys.stderr)

    for filename, line_number, version, line in versions:
        print(f'{filename}:{line_number}: {line}', file=sys.stderr)

    sys.exit(1)

print(f'Versions number match.  All good!', file=sys.stderr)
