#!/usr/bin/env python3
# Copyright (c) 2022 L. E. Segovia <amy@amyspark.me>
#
# This file is part of the FFmpeg Meson build
#
# This library is free software; you can redistribute it and/or
# modify it under the terms of the GNU Lesser General Public
# License as published by the Free Software Foundation; either
# version 2.1 of the License, or (at your option) any later version.
#
# This library is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
# Lesser General Public License for more details.
#
# You should have received a copy of the GNU Lesser General Public
# License along with this library; if not, see <http://www.gnu.org/licenses/>.

import argparse
import errno
import os
import pathlib
import re
import subprocess


def output(platform, symbols):
    if platform == 'win':
        print("EXPORTS")
        print(*[f'    {symbol}' for symbol in sorted(set(symbols))], sep='\n')
    elif platform == 'darwin':
        print(*[f'{prefix}{symbol}' for symbol in sorted(set(symbols))], sep='\n')
    else:
        print('{')
        print('    global:')
        print(
            *[f'        {prefix}{symbol};' for symbol in sorted(set(symbols))], sep='\n')
        print('    local:')
        print('        *;')
        print('};')


if __name__ == '__main__':
    arg_parser = argparse.ArgumentParser(
        description='Craft the symbols exports file')

    arg_parser.add_argument('--prefix', metavar='PREFIX',
                            help='Prefix for extern symbols')
    g = arg_parser.add_argument_group('Library parsing tool')
    group = g.add_mutually_exclusive_group(required=True)
    group.add_argument('--nm', metavar='NM_PATH', type=pathlib.Path,
                       help='If specified, runs this instead of dumpbin (MinGW)')
    group.add_argument('--dumpbin', metavar='DUMPBIN_PATH', type=pathlib.Path,
                       help='If specified, runs this instead of nm (MSVC)')
    group.add_argument(
        '--list', action='store_true', help='If specified, consider FILE as an exported symbols list instead of a library')
    g = arg_parser.add_argument_group('Symbol naming')
    group = g.add_mutually_exclusive_group(required=True)
    group.add_argument('--regex', metavar='REGEX', type=str,
                       nargs='+',
                       help='Regular expression for exported symbols')
    group.add_argument('--vscript', metavar='VERSION_SCRIPT',
                       type=argparse.FileType('r'), help='Version script')
    arg_parser.add_argument('--os', type=str, choices=('win', 'linux', 'darwin'),
                            default='linux', required=True,
                            help='Target operating system for the exports file (win = MSVC module definition file, linux = version script, darwin = exported symbols list)')
    arg_parser.add_argument('libnames', metavar='FILE', type=pathlib.Path,
                            nargs='+',
                            help='Source file(s) to parse')

    args = arg_parser.parse_args()

    libnames = args.libnames

    for libname in libnames:
        if not libname.exists():
            raise FileNotFoundError(
                errno.ENOENT, os.strerror(errno.ENOENT), libname)

    if not args.list and len(libnames) > 1:
        raise ValueError("Expect 1 filename as argument.")

    prefix = args.prefix or ''
    started = 0
    regex = []

    if args.vscript:
        for line in args.vscript:
            # We only care about global symbols
            if re.match(r'^\s+global:', line):
                started = 1
                line = re.sub(r'^\s+global: *', '', line)
            else:
                if re.match(r'^\s+local:', line):
                    started = 0

            if started == 0:
                continue

            line = line.replace(';', '')

            for exp in line.split():
                # Remove leading and trailing whitespace
                regex.append(exp.strip())
    else:
        regex.extend(args.regex)

    # Ensure things are compatible on Windows with Python 3.7.x
    libname_path_posix = pathlib.Path(libnames[0]).as_posix()

    if args.list:
        dump = []
        for libname in libnames:
            syms = libname.open('r', encoding='utf-8').readlines()
            # Strip whitespaces
            syms = [x.strip() for x in syms]
            # Exclude blank lines
            syms = [x for x in syms if len(x) > 0]
            dump.extend(syms)
    elif args.nm is not None:
        # Use eval, since NM="nm -g"
        # Add -j to ensure only symbol names are output (otherwise in macOS
        # a race condition can occur in the redirection)
        # And use `--no-llvm-bc` in case it's /usr/bin/nm on macOS
        s = subprocess.run(
            [args.nm, '-U', '-g', '-j', '--no-llvm-bc', libname_path_posix],
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            universal_newlines=True,
            check=False,
        )
        if s.returncode != 0:
            # If it fails, retry with --defined-only (non macOS)
            s = subprocess.run(
                [args.nm, '--defined-only', '-g', '-j', '--no-llvm-bc', libname_path_posix],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                universal_newlines=True,
                check=False,
            )
        if s.returncode != 0:
            # If it fails, retry without skipping LLVM bitcode (macOS flag)
            # Don't use -U, as that was an alias for --unicode= instead of
            # --defined-only before Binutils 2.39
            s = subprocess.run(
                [args.nm, '--defined-only', '-g', '-j', libname_path_posix],
                stdout=subprocess.PIPE,
                stderr=subprocess.STDOUT,
                universal_newlines=True,
                check=False,
            )
        if s.returncode != 0:
            # -j was added only in Binutils 2.37
            s = subprocess.run(
                [args.nm, '--defined-only', '-g', libname_path_posix],
                stdout=subprocess.PIPE,
                stderr=subprocess.DEVNULL,
                universal_newlines=True,
                check=True,
            )
        dump = s.stdout.splitlines()
        # Exclude lines with ':' (object name)
        dump = [x for x in dump if ":" not in x]
        # Exclude blank lines
        dump = [x for x in dump if len(x) > 0]
        # Subst the prefix out
        dump = [re.sub(f'^{prefix}', '', x) for x in dump]
    else:
        dump = subprocess.run([pathlib.Path(args.dumpbin).as_posix(), '-linkermember:1', libname_path_posix],
                              stdout=subprocess.PIPE, stderr=subprocess.STDOUT, universal_newlines=True).stdout.splitlines()
        # Find the index of the first line with
        # "public symbols", keep the rest
        # Then the line with " Summary",
        # delete it and the rest
        for i, line in enumerate(dump):
            if 'public symbols' in line:
                start = i
            elif re.match(r'\s+Summary', line):
                end = i
        dump = dump[start:end]
        # Substitute prefix out
        dump = [re.sub(fr'\s+{prefix}', ' ', x) for x in dump]
        # Substitute big chonky spaces out
        dump = [re.sub(r'\s+', ' ', x) for x in dump]
        # Exclude blank lines
        dump = [x for x in dump if len(x) > 0]
        # Take only the *second* field (split by spaces)
        # Python's split excludes whitespace at the beginning
        dump = [x.split()[1] for x in dump]

    symbols = []
    for exp in regex:
        for i in dump:
            if re.match(exp, i):
                symbols.append(i)

    output(args.os, symbols)
