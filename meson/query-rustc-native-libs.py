#!/usr/bin/env python3

from argparse import ArgumentParser
from pathlib import Path
import re
import subprocess
import tempfile

parser = ArgumentParser()

parser.add_argument("RUSTC", type=Path, help="Path to rustc")

if __name__ == "__main__":
    args = parser.parse_args()

    with tempfile.TemporaryDirectory() as tmpdir:
        dummy_rs = Path(tmpdir) / "dummy.rs"
        dummy_rs.open("w", encoding="utf-8").write(
            """
            use std::io::{Result};
            pub fn main() -> Result<()>{
                println!("Hello world!");
                Ok(())
            }
        """
        )

        native_static_libs = subprocess.run(
            [
                args.RUSTC,
                "--print=native-static-libs",
                "--crate-type",
                "staticlib",
                dummy_rs,
            ],
            check=True,
            capture_output=True,
            text=True,
            cwd=tmpdir,
        )
        for i in native_static_libs.stderr.strip().splitlines():
            match = re.match(r".+native-static-libs: (.+)", i)
            if match:
                print(
                    " ".join(
                        set(
                            [lib.removesuffix(".lib") for lib in match.group(1).split()]
                        )
                    )
                )
