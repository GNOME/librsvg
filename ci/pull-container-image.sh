#!/bin/bash
#
# Utility script so you can pull the container image from CI for local development.
# Run this script and follow the instructions; the script will tell you how
# to run "podman run" to launch a container that has the same environment as the
# one used during CI pipelines.  You can debug things at leisure there.

set -eu
set -o pipefail

CONTAINER_BUILDS=ci/container_builds.yml

if [ ! -f $CONTAINER_BUILDS ]
then
    echo "Please run this from the toplevel source directory in librsvg"
    exit 1
fi

tag=$(grep -e '^  BASE_TAG:' $CONTAINER_BUILDS | head -n 1 | sed -E 's/.*BASE_TAG: "(.+)"/\1/')
rust_version=$(grep -e '^  RUST_STABLE:' $CONTAINER_BUILDS | head -n 1 | sed -E 's/.*RUST_STABLE: "(.+)"/\1/')
full_tag=x86_64-$rust_version-$tag

image_name=registry.gitlab.gnome.org/gnome/librsvg/opensuse/tumbleweed:$full_tag

echo pulling image "$image_name"
podman pull "$image_name"

echo ""
echo "You can now run this:"
echo "  podman run --rm -ti --cap-add=SYS_PTRACE -v \$(pwd):/srv/project:z -w /srv/project $image_name"
echo ""
echo "Don't forget to run this once inside the container:"
echo "  source ci/env.sh"
echo "  source ci/setup-dependencies-env.sh"
