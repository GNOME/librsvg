#!/usr/bin/env python3

from argparse import ArgumentParser
from pathlib import Path
import os
import re
import subprocess
import sys
import tempfile

parser = ArgumentParser()

parser.add_argument("RUSTC", type=Path, help="Path to rustc")
parser.add_argument("--query", action="store",
                    choices=['native-static-libs', 'default-host-toolchain', 'stable-actual-version'],
                    help="Item to query from RustC ['native-static-libs', 'default-host-toolchain', 'stable-actual-version']",
                    required=True)
parser.add_argument("--toolchain-version", action="store",
                    help="Rust Toolchain Version (if needed)")
parser.add_argument("--target", help="Target triplet")
parser.add_argument("--build-triplet", help="Build machine triplet (for cross builds using specific toolchain version)")

def removeprefix_fallback(s, pfx):
    if sys.version_info > (3, 9):
        return s.removeprefix(pfx)
    elif s.startswith(pfx):
        return s[len(pfx):]
    else:
        return s

def removesuffix_fallback(s, sfx):
    if sys.version_info > (3, 9):
        return s.removesuffix(sfx)
    elif s.endswith(sfx):
        return s[:-len(sfx)]
    else:
        return s

def retrieve_native_static_libs_from_output(output):
    if re.match(r'^error[:|\[]', output):
        print(output, file=sys.stderr)
        sys.exit()

    for i in output.strip().splitlines():
        match = re.match(r".+native-static-libs: (.+)", i)
        if match:
            libs = match.group(1).split()
            libs = [removesuffix_fallback(lib, ".lib") for lib in libs] # msvc
            libs = [removeprefix_fallback(lib, "-l") for lib in libs] # msys2
            print(
                " ".join(
                    set(
                        libs
                    )
                )
            )

# Get the default target host or actual version of toolchain
def retrive_version_info(output, query):
    for i in output.strip().splitlines():
        match = re.match(r"%s: (.+)" % query, i)
        if match:
            return match.group(1)

if __name__ == "__main__":
    args = parser.parse_args()
    query = args.query
    query_arg = None
    rustc_cmd = [Path(args.RUSTC).as_posix()]

    if args.toolchain_version is not None:
        if args.target is None and args.build_triplet is None:
            raise ValueError('--target or --build-triplet argument required if --toolchain-version is used')
        if args.build_triplet is not None:
            rustc_cmd.extend(['+%s-%s' % (args.toolchain_version, args.build_triplet)])
        else:
            rustc_cmd.extend(['+%s-%s' % (args.toolchain_version, args.target)])
        
    if query == 'native-static-libs':
        query_arg = ['--print=%s' % query]
    else:
        query_arg = ['--version', '--verbose']
    rustc_cmd.extend(query_arg)
    if args.target:
        rustc_cmd.extend(['--target', args.target])

    fd, dummy_out = tempfile.mkstemp()
    os.close(fd)
    try:
        # We need these for '--print=native-static-libs' on Windows
        if query == 'native-static-libs':
            rustc_cmd.extend(['--crate-type', 'staticlib'])
            rustc_cmd.append(os.devnull)
            rustc_cmd.extend(['-o', dummy_out])

        query_results = subprocess.run(
            rustc_cmd,
            capture_output=True,
            text=True,
        )
    finally:
        os.unlink(dummy_out)

    if query == 'native-static-libs':
        retrieve_native_static_libs_from_output(query_results.stderr)
    elif query == 'default-host-toolchain' or query == 'stable-actual-version':
        if query_results.stderr == '':
            if query == 'default-host-toolchain':
                result = retrive_version_info(query_results.stdout, 'host')
            else:
                result = retrive_version_info(query_results.stdout, 'release')
            print(result)
        else:
            print(query_results.stderr, file=sys.stderr)
