#!/bin/bash
#
# Creates a Python virtual environment in /usr/local/python and installs
# the modules from requirements.txt in it.  These modules are required
# by various jobs in the CI pipeline.
#
# IMPORTANT: See
# https://gnome.pages.gitlab.gnome.org/librsvg/devel-docs/ci.html#container-image-version

set -eux -o pipefail

python3 -m venv /usr/local/python
source /usr/local/python/bin/activate
pip3 install --upgrade pip
pip3 install -r ci/requirements.txt
