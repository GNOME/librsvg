#!/usr/bin/python
#
# Utility script to generate .pc files for GLib
# for Visual Studio builds, to be used for
# building introspection files

# Author: Fan, Chun-wei
# Date: March 10, 2016

import os
import sys

from replace import replace_multi
from pc_base import BasePCItems

def main(argv):
    rsvg_api_ver = '2.0'
    base_pc = BasePCItems()

    base_pc.setup(argv)
    pkg_replace_items = {'@RSVG_API_MAJOR_VERSION@': rsvg_api_ver,
                         '@RSVG_API_VERSION@': rsvg_api_ver,
#                         ' cairo': '',
                         '-lm': '-lcairo'}

    pkg_replace_items.update(base_pc.base_replace_items)

    # Generate librsvg-$(rsvg_api_ver).pc
    replace_multi(base_pc.top_srcdir + '/librsvg.pc.in',
                  base_pc.srcdir + '/librsvg-' + rsvg_api_ver + '.pc',
                  pkg_replace_items)

if __name__ == '__main__':
    sys.exit(main(sys.argv))
