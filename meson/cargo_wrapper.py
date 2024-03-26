#!/usr/bin/env python3

from argparse import ArgumentParser
import os
from pathlib import Path
import shutil
import subprocess
import sys

parser = ArgumentParser("Cargo wrapper")

parser.add_argument(
    "--command",
    required=True,
    choices=["cbuild", "test", "build"],
    help="Cargo command",
)

parser.add_argument(
    "--cargo", required=True, type=Path, help="Path to the cargo executable"
)

parser.add_argument(
    "--manifest-path", required=True, type=Path, help="Path to Cargo.toml"
)

parser.add_argument(
    "--current-build-dir",
    required=True,
    type=Path,
    help="Value from meson.current_build_dir()",
)

parser.add_argument(
    "--current-source-dir",
    required=True,
    type=Path,
    help="Value from meson.current_source_dir()",
)

parser.add_argument(
    "--project-build-root",
    required=True,
    type=Path,
    help="Value from meson.project_build_root()",
)

parser.add_argument(
    "--toolchain-version", help="Rust Toolchain Version if needed"
)

parser.add_argument(
    "--target", help="Target triplet"
)

parser.add_argument(
    "--build-triplet", help="Build toolchain triplet (for cross builds using specific toolchain version)"
)

parser.add_argument(
    "--release", action="store_true", help="Build artifacts in release mode"
)

parser.add_argument(
    "--packages",
    nargs="*",
    default=[],
    help='Rust packages to build (names for "cargo cbuild/build -p")',
)

parser.add_argument(
    "--prefix", type=Path, required=True, help="Value of get_option('prefix')"
)

parser.add_argument("--libdir", required=True, help="Value of get_option('libdir')")

g = parser.add_argument_group("Outputs")
group = parser.add_mutually_exclusive_group(required=False)
group.add_argument(
    "--extension", help="filename extension for the library (so, a, dll, lib, dylib)",
)
group.add_argument("--bin", help="Name of binary to build")

args = parser.parse_args()

if args.toolchain_version is not None and args.target is None and args.build_triplet is None:
    raise ValueError('--target and/or --build-triplet argument required if --toolchain-version is specified')

if args.command == 'test':
    if args.extension or args.bin:
        raise ValueError('Cargo test does not take --extension or --bin')

cargo_target_dir = Path(args.project_build_root) / "target"

# The final rsvg-convert executable will be found in cargo_target_dir/$(TARGET_TRIPLET)
# if a target triplet is specified
if args.target:
    cargo_target_output_dir = cargo_target_dir / args.target
else:
    cargo_target_output_dir = cargo_target_dir

env = os.environ.copy()
pkg_config_path = [i for i in env.get("PKG_CONFIG_PATH", "").split(os.pathsep) if i]
pkg_config_path.insert(
    0, (Path(args.project_build_root) / "meson-uninstalled").as_posix()
)
env["PKG_CONFIG_PATH"] = os.pathsep.join(pkg_config_path)

cargo_prefixes = [
    "--prefix",
    Path(args.prefix).as_posix(),
    "--libdir",
    (Path(args.prefix) / args.libdir).as_posix(),
]

cargo_cmd = [Path(args.cargo).as_posix()]

if args.toolchain_version is not None:
    if args.build_triplet is not None:
        cargo_cmd.extend(["+%s-%s" % (args.toolchain_version, args.build_triplet)])
    else:
        cargo_cmd.extend(["+%s-%s" % (args.toolchain_version, args.target)])

if args.command == "cbuild":
    cargo_cmd.extend(["cbuild", "--locked"])
    library_type = "staticlib" if args.extension in ("a", "lib") else "cdylib"
    cargo_cmd.extend(cargo_prefixes)
    cargo_cmd.extend(["--library-type", library_type])
elif args.command == "test":
    cargo_cmd.extend(["test", "--locked", "--no-fail-fast", "--color=always"])
    if 'librsvg' in args.packages:
        cargo_cmd.extend([
            "--features",
            # These are required for librsvg itself
            # If doing an unqualified cargo build, they'll be called up
            # by rsvg-convert
            # https://github.com/rust-lang/cargo/issues/2911
            "capi,test-utils",
        ])
else:
    cargo_cmd.extend(["build", "--locked"])
    if args.bin:
        cargo_cmd.extend(["--bin", args.bin])

cargo_cmd.extend(["--manifest-path", Path(args.manifest_path).as_posix()])
cargo_cmd.extend(["--target-dir", cargo_target_dir.as_posix()])

if args.release:
    buildtype = 'release'
    cargo_cmd.extend(['--release'])
else:
    buildtype = 'debug'

if args.target:
    cargo_cmd.extend(['--target', args.target])

for p in args.packages:
    cargo_cmd.extend(["-p", p])

if args.command == "test":
    cargo_cmd.extend(["--", "--include-ignored"])

print(f"command: {cargo_cmd}")
subprocess.run(cargo_cmd, env=env, check=True)

if args.command in ["cbuild", "build"]:
    # Copy so/dll/etc files to build dir
    if args.extension:
        for f in cargo_target_dir.glob(f"**/{buildtype}/*.{args.extension}"):
            shutil.copy(f, args.current_build_dir)
    # Copy binary and, if applicable, the corresponding .pdb file, to build dir
    else:
        binary = Path(cargo_target_output_dir / buildtype / args.bin)
        if sys.platform == "win32":
            pdb_copy = Path(cargo_target_output_dir / buildtype / args.bin.replace('rsvg-convert', 'rsvg_convert')).with_suffix(".pdb")
            if os.path.exists(pdb_copy):
                shutil.copy(pdb_copy, args.current_build_dir)
            binary = binary.with_suffix(".exe")
        shutil.copy(binary, args.current_build_dir)
