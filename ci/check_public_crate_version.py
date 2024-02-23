# Checks that the version of the librsvg public crate matches the version for the GNOME library.
#
# For stable releases:
#   - GNOME: 2.57.2
#   - crate: 2.57.2
#
# For development relases, .9x vs. -beta.x
#   - GNOME: 2.57.90
#   - crate: 2.58.0-beta.0

import semver
import sys
import toml

from utils import get_project_version_str

def gen_crate_version_from_project_version(v):
    if v.patch < 90:
        # stable release, just return it
        return v
    elif v.patch >= 90 and v.patch < 99:
        # development release, mangle it for semver
        patch_level = v.patch - 90
        beta = f'beta.{patch_level}'
        return v.bump_minor().replace(prerelease = beta)
    else:
        raise Exception("don't know what to do with patch versions larger than 99")

def check_crate_version(project_version_str, crate_version_str):
    # GNOME only likes x.y.z versions
    main_version = semver.Version.parse(project_version_str)
    assert main_version.major is not None
    assert main_version.minor is not None
    assert main_version.patch is not None
    assert main_version.prerelease is None
    assert main_version.build is None

    crate_version = semver.Version.parse(crate_version_str)

    if gen_crate_version_from_project_version(main_version) != crate_version:
        raise Exception(
            f'meson.build version {main_version} does not match rsvg crate version {crate_version}'
        )

def test_stable():
    a = semver.Version.parse('2.56.3')
    assert gen_crate_version_from_project_version(a) == a

def test_development():
    a = semver.Version.parse('2.56.90')
    assert gen_crate_version_from_project_version(a) == semver.Version.parse('2.57.0-beta.0')

    a = semver.Version.parse('2.57.93')
    assert gen_crate_version_from_project_version(a) == semver.Version.parse('2.58.0-beta.3')

def main():
    project_version_str = get_project_version_str()

    doc = toml.load('rsvg/Cargo.toml')
    crate_version_str = doc['package']['version']

    check_crate_version(project_version_str, crate_version_str)
    print('Versions number match.  All good!', file=sys.stderr)

if __name__ == "__main__":
    main()
