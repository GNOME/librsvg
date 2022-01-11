#!/usr/bin/python
# vim: encoding=utf-8
#expand *.in files
#this script is only intended for building from git, not for building from the released tarball, which already includes all necessary files
import os
import sys
import re
import string
import subprocess
import optparse

def get_version(srcroot):
    ver = {}
    RE_VERSION = re.compile(r'^m4_define\(\[(rsvg_\w+)\],\s*\[(\d+)\]\)')
    with open(os.path.join(srcroot, 'configure.ac'), 'r') as ac:
        for i in ac:
            mo = RE_VERSION.search(i)
            if mo:
                ver[mo.group(1).upper()] = int(mo.group(2))
    ver['LIBRSVG_MAJOR_VERSION'] = ver['RSVG_MAJOR_VERSION']
    ver['LIBRSVG_MINOR_VERSION'] = ver['RSVG_MINOR_VERSION']
    ver['LIBRSVG_MICRO_VERSION'] = ver['RSVG_MICRO_VERSION']
    ver['PACKAGE_VERSION'] = '%d.%d.%d' % (ver['LIBRSVG_MAJOR_VERSION'],
                                           ver['LIBRSVG_MINOR_VERSION'],
                                           ver['LIBRSVG_MICRO_VERSION'])
    ver['PACKAGE'] = 'librsvg'
    ver['PACKAGE_NAME'] = ver['PACKAGE']
    ver['PACKAGE_TARNAME'] = ver['PACKAGE']
    ver['GETTEXT_PACKAGE'] = ver['PACKAGE']
    ver['PACKAGE_BUGREPORT'] = 'https://gitlab.gnome.org/GNOME/librsvg/issues'
    return ver

def process_in(src, dest, vars):
    RE_VARS = re.compile(r'@(\w+?)@')
    with open(src, 'r') as s:
        with open(dest, 'w') as d:
            for i in s:
                i = RE_VARS.sub(lambda x: str(vars[x.group(1)]), i)
                d.write(i)

def get_srcroot():
    if not os.path.isabs(__file__):
        path = os.path.abspath(__file__)
    else:
        path = __file__
    dirname = os.path.dirname(path)
    return os.path.abspath(os.path.join(dirname, '..'))

def main(argv):
    srcroot = get_srcroot()
    ver = get_version(srcroot)
    process_in('config.h.win32.in', 'config.h.win32', ver.copy())
    process_in('config-msvc.mak.in', 'config-msvc.mak', ver.copy())
    process_in(os.path.join(srcroot, 'include', 'librsvg', 'rsvg-version.h.in'),
               os.path.join(srcroot, 'include', 'librsvg', 'rsvg-version.h'),
               ver.copy())

if __name__ == '__main__':
    sys.exit(main(sys.argv))
