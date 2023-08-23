# This script checks that the project's version is the same in a few files where it must appear.

import re
import sys
import toml

def get_first_group(regex, line):
    matches = regex.search(line)
    if matches is None:
        return None
    else:
        return matches.group(1)

def get_configure_ac_version():
    major_regex = re.compile(r'^m4_define\(\[rsvg_major_version\],\[(\d+)\]\)')
    minor_regex = re.compile(r'^m4_define\(\[rsvg_minor_version\],\[(\d+)\]\)')
    micro_regex = re.compile(r'^m4_define\(\[rsvg_micro_version\],\[(\d+)\]\)')

    major = None
    micro = None
    minor = None

    with open("configure.ac") as f:
        for line in f.readlines():
            if major is None:
                major = get_first_group(major_regex, line)

            if minor is None:
                minor = get_first_group(minor_regex, line)

            if micro is None:
                micro = get_first_group(micro_regex, line)

    if not (major and minor and micro):
        raise Exception('configure.ac does not have all the necessary version numbers')
            
    return f'{major}.{minor}.{micro}'

def get_cargo_toml_version():
    doc = toml.load('Cargo.toml')
    return doc['workspace']['package']['version']

def get_doc_version():
    doc = toml.load('doc/librsvg.toml')
    return doc['library']['version']

versions = [
    ['configure.ac', get_configure_ac_version()],
    ['Cargo.toml', get_cargo_toml_version()],
    ['doc/librsvg.toml', get_doc_version()],
]

all_the_same = True

for filename, version in versions[1:]:
    if version != versions[0][1]:
        all_the_same = False

if not all_the_same:
    print(f'Version numbers do not match, please fix them!\n', file=sys.stderr)
    for filename, version in versions:
        print(f'{filename}: {version}', file=sys.stderr)

    sys.exit(1)

print(f'Versions number match.  All good!', file=sys.stderr)
