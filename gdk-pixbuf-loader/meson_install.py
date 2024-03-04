#!/usr/bin/env python3

from argparse import ArgumentParser
from pathlib import Path
import os
import subprocess
import sys

argparse = ArgumentParser('Deploy loaders.cache')

argparse.add_argument('--gdk-pixbuf-moduledir', type=Path)
argparse.add_argument('gdk_pixbuf_queryloaders', type=Path)
argparse.add_argument('gdk_pixbuf_cache_file', type=Path)
argparse.add_argument('--show-cross-message', action='store_true')

if __name__ == '__main__':
    args = argparse.parse_args()

    cache_file: Path = args.gdk_pixbuf_cache_file

    # Install the files relative to the destdir if it's set
    destdir = os.environ.get("DESTDIR")
    if destdir is not None:
        destdir: Path = Path(destdir)
        # Make sure it's a valid Path object
        assert destdir is not None
        cache_file = destdir / cache_file.relative_to(cache_file.anchor)

    if args.show_cross_message:
        print('*** Note: Please run gdk-pixbuf-queryloaders manually ' +
              'against the newly-built gdkpixbuf-svg loader', file=sys.stderr)
    else:
        with cache_file.open('w', encoding='utf-8') as f:
            subprocess.run(
                [args.gdk_pixbuf_queryloaders],
                env={
                    'GDK_PIXBUF_MODULEDIR': args.gdk_pixbuf_moduledir
                },
                stdout=f,
                check=True
            )

