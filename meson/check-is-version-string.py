#!/usr/bin/env python3

# Simple script to check whether rustc-version is to be checkedn against minimum supported rust version

import sys
from argparse import ArgumentParser

if __name__ == "__main__":
    parser = ArgumentParser()
    parser.add_argument('--string', help='String to check is a version-like string', required=True)
    args = parser.parse_args()
    parts = args.string.split('.')
    if len(parts) != 2 and len(parts) != 3:
        print('skip')
    else:
        for p in parts:
            try:
                int(p)
            except ValueError:
                print('skip')
                sys.exit()
        print('check')
