#!/usr/bin/env python3

# This file is part of The Croco Library
# This program is free software; you can redistribute it and/or
# modify it under the terms of version 2.1 of the GNU Lesser General Public
# License as published by the Free Software Foundation.

# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU General Public License for more details.

# You should have received a copy of the GNU Lesser General Public License
# along with this program; if not, write to the Free Software
# Foundation, Inc., 59 Temple Place, Suite 330, Boston, MA 02111-1307
# USA

# Author: Fan, Chun-wei
# See COPYRIGHTS file for copyright information.

import os
import sys

if len(sys.argv) != 3:
    raise ValueError('Usage: %s <input> <output>' % sys.argv[0])

input = sys.argv[1]
output = sys.argv[2]

if not os.path.isfile(input):
    raise ValueError('Input file %s does not exist' % input)

outdir = os.path.dirname(output)
if not outdir == '' and not os.path.isdir(os.path.dirname(output)):
    raise ValueError('Ensure directory for %s is created' % output)

with open(output, 'w') as o:
    o.write('EXPORTS')
    with open(input) as i:
        for l in i:
            o.write(l)
    