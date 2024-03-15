#!/usr/bin/env python3

from argparse import ArgumentParser
from pathlib import Path
import os
import re
import subprocess
import tempfile

parser = ArgumentParser()

parser.add_argument("RUSTC", type=Path, help="Path to rustc")

parser.add_argument("--target", help="Target triplet")

if __name__ == "__main__":
    args = parser.parse_args()
    dummy_out = tempfile.NamedTemporaryFile()

    rustc_cmd = [args.RUSTC, "--print=native-static-libs", "--crate-type", "staticlib"]
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
            libs = [lib.removesuffix(".lib") for lib in libs] # msvc
            libs = [lib.removeprefix("-l") for lib in libs] # msys2
            print(
                " ".join(
                    set(
                        libs
                    )
                )
            )
