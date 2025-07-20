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
    "--avif", action="store_true", help="Enable AVIF support"
)

parser.add_argument(
    "--pixbuf", action="store_true", help="Enable GDK-Pixbuf support"
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

g = parser.add_argument_group("Optimizations")
g.add_argument(
    "--release", action="store_true", help="Build artifacts in release mode"
)
g.add_argument(
    '--optimization', choices=['0', '1', '2', '3', 's'], help="Set optimization level"
)
g.add_argument(
    '--lto', choices=['fat', 'thin'], help="Set optimization level"
)

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

features = []

if args.avif:
    features.append('avif')

if args.pixbuf:
    features.append('pixbuf')

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
        # These are required for librsvg itself
        # If doing an unqualified cargo build, they'll be called up
        # by rsvg-convert
        # https://github.com/rust-lang/cargo/issues/2911
        features.extend(["capi", "test-utils"])
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

if args.optimization:
    env[f'CARGO_PROFILE_{buildtype.upper()}_OPT_LEVEL'] = args.optimization
if args.lto:
    env[f'CARGO_PROFILE_{buildtype.upper()}_CODEGEN_UNITS'] = '1'
    env[f'CARGO_PROFILE_{buildtype.upper()}_LTO'] = args.lto

if args.target:
    cargo_cmd.extend(['--target', args.target])

if features:
    cargo_cmd.extend(["--features", ",".join(features)])

for p in args.packages:
    cargo_cmd.extend(["-p", p])

if args.command == "test":
    cargo_cmd.extend(["--", "--include-ignored"])

k = {k: v for k, v in env.items() if k.startswith('CARGO_PROFILE')}
print(f"command: {cargo_cmd}, env: {k}")
subprocess.run(cargo_cmd, env=env, check=True)

if args.command in ["cbuild", "build"]:
    # Copy so/dll/etc files to build dir
    if args.extension:
        for f in cargo_target_dir.glob(f"**/{buildtype}/*.{args.extension}"):
            shutil.copy(f, args.current_build_dir)
    # Copy binary and, if applicable, the corresponding .pdb file, to build dir
    else:
        binary = Path(cargo_target_output_dir / buildtype / args.bin)
        is_windows_target = False
        if args.target is None and sys.platform == "win32":
            is_windows_target = True
        elif args.target is not None and args.target.split('-')[2] == 'windows':
            is_windows_target = True
        if is_windows_target:
            exe_name = args.bin.replace('rsvg-convert', 'rsvg_convert')
            pdb_src = Path(cargo_target_output_dir / buildtype / exe_name).with_suffix(".pdb")
            pdb_dest = Path(args.current_build_dir / args.bin).with_suffix('.pdb')
            if pdb_src.exists():
                shutil.copy(pdb_src, pdb_dest)
            binary = binary.with_suffix(".exe")
        shutil.copy(binary, args.current_build_dir)
