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

    if args.show_cross_message or os.environ.get("DESTDIR"):
        print('*** Note: Please run gdk-pixbuf-queryloaders manually ' +
              'against the newly-built gdkpixbuf-svg loader', file=sys.stderr)
    else:
        env = os.environ.copy()
        env['GDK_PIXBUF_MODULEDIR'] = Path(args.gdk_pixbuf_moduledir).as_posix()
        with cache_file.open('w', encoding='utf-8') as f:
            subprocess.run(
                [Path(args.gdk_pixbuf_queryloaders).as_posix()],
                env=env,
                stdout=f,
                check=True
            )

