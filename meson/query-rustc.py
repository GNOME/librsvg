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
                    choices=['native-static-libs'],
                    help="Item to query from RustC ['native-static-libs']",
                    required=True)

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

parser.add_argument("--target", help="Target triplet")

if __name__ == "__main__":
    args = parser.parse_args()
    dummy_out = tempfile.NamedTemporaryFile()
    query = args.query
    query_arg = None

    if query == 'native-static-libs':
        query_arg = ['--print=%s' % query]
    rustc_cmd = [
        Path(args.RUSTC).as_posix(),
    ]
    rustc_cmd.extend(query_arg)
    if args.target:
        rustc_cmd.extend(['--target', args.target])

    # We need these for '--print=native-static-libs' on Windows
    if query == 'native-static-libs':
        rustc_cmd.extend(['--crate-type', 'staticlib'])
        rustc_cmd.append(os.devnull)
        rustc_cmd.extend(['-o', dummy_out.name])

    query_results = subprocess.run(
        rustc_cmd,
        capture_output=True,
        text=True,
    )
    if query == 'native-static-libs':
        retrieve_native_static_libs_from_output(query_results.stderr)
