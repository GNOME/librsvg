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

parser.add_argument("--target", help="Target triplet")

if __name__ == "__main__":
    args = parser.parse_args()
    dummy_out = tempfile.NamedTemporaryFile()

    rustc_cmd = [
        Path(args.RUSTC).as_posix(),
        "--print=native-static-libs",
        "--crate-type", "staticlib"
    ]
    if args.target:
        rustc_cmd.extend(['--target', args.target])

    rustc_cmd.append(os.devnull)
    rustc_cmd.extend(['-o', dummy_out.name])

    native_static_libs = subprocess.run(
        rustc_cmd,
        capture_output=True,
        text=True,
    )
    for i in native_static_libs.stderr.strip().splitlines():
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
