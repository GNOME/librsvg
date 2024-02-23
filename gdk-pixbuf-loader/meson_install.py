#!/usr/bin/env python3

from argparse import ArgumentParser
from pathlib import Path
import subprocess

argparse = ArgumentParser('Deploy loaders.cache')

argparse.add_argument('--gdk-pixbuf-moduledir', type=Path)
argparse.add_argument('gdk_pixbuf_queryloaders', type=Path)
argparse.add_argument('gdk_pixbuf_cache_file', type=Path)

if __name__ == '__main__':
    args = argparse.parse_args()

    cache_file: Path = args.gdk_pixbuf_cache_file

    with cache_file.open('w', encoding='utf-8') as f:
        subprocess.run(
            [args.gdk_pixbuf_queryloaders],
            env={
                'GDK_PIXBUF_MODULEDIR': args.gdk_pixbuf_moduledir
            },
            stdout=f,
            capture_output=True,
            check=True
        )
