# This script checks that the project's version is the same in a few files where it must appear.

import sys
import toml

from utils import get_project_version_str

def get_cargo_toml_version():
    doc = toml.load('Cargo.toml')
    return doc['workspace']['package']['version']

def get_doc_version():
    doc = toml.load('doc/librsvg.toml')
    return doc['library']['version']

def main():
    versions = [
        ['meson.build', get_project_version_str()],
        ['Cargo.toml', get_cargo_toml_version()],
        ['doc/librsvg.toml', get_doc_version()],
    ]

    all_the_same = True

    for filename, version in versions[1:]:
        if version != versions[0][1]:
            all_the_same = False

    if not all_the_same:
        print('Version numbers do not match, please fix them!\n', file=sys.stderr)
        for filename, version in versions:
            print(f'{filename}: {version}', file=sys.stderr)

        sys.exit(1)

    print('Versions number match.  All good!', file=sys.stderr)

if __name__ == "__main__":
    main()
