#!/usr/bin/env python3

from argparse import ArgumentParser
from pathlib import Path
import subprocess
import os

argparse = ArgumentParser('Deploy loaders.cache')

argparse.add_argument('--gdk-pixbuf-moduledir', type=Path)
argparse.add_argument('gdk_pixbuf_queryloaders', type=Path)
argparse.add_argument('gdk_pixbuf_cache_file', type=Path)

if __name__ == '__main__':
    args = argparse.parse_args()

    cache_file: Path = args.gdk_pixbuf_cache_file

    # Install the files relative to the destdir if it's set
    destdir = os.environ.get("DESTDIR")
    if destdir is not None:
        destdir = Path(destdir)
        # Make sure it's a valid Path object
        assert destdir is not None
        cache_file = destdir / cache_file.relative_to(cache_file.anchor)

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

