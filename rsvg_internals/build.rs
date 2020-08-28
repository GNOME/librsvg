use std::env;
use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;
use std::process;

use pkg_config::{Config, Error};

fn main() {
    find_libxml2();
    generate_srgb_tables();
}

fn find_libxml2() {
    if let Err(s) = find("libxml-2.0", "2.9.0", &["xml2"]) {
        let _ = writeln!(io::stderr(), "{}", s);
        process::exit(1);
    }
}

// This is stolen from the -sys crates in gtk-rs
fn find(package_name: &str, version: &str, shared_libs: &[&str]) -> Result<(), Error> {
    if let Ok(inc_dir) = env::var("GTK_INCLUDE_DIR") {
        println!("cargo:include={}", inc_dir);
    }
    if let Ok(lib_dir) = env::var("GTK_LIB_DIR") {
        for lib_ in shared_libs.iter() {
            println!("cargo:rustc-link-lib=dylib={}", lib_);
        }
        println!("cargo:rustc-link-search=native={}", lib_dir);
        return Ok(());
    }

    let target = env::var("TARGET").unwrap();
    let hardcode_shared_libs = target.contains("windows");

    let mut config = Config::new();
    config.atleast_version(version);
    config.print_system_libs(false);

    if hardcode_shared_libs {
        config.cargo_metadata(false);
    }
    match config.probe(package_name) {
        Ok(library) => {
            if let Ok(paths) = std::env::join_paths(library.include_paths) {
                // Exposed to other build scripts as DEP_CAIRO_INCLUDE; use env::split_paths
                println!("cargo:include={}", paths.to_string_lossy());
            }
            if hardcode_shared_libs {
                for lib_ in shared_libs.iter() {
                    println!("cargo:rustc-link-lib=dylib={}", lib_);
                }
                for path in library.link_paths.iter() {
                    println!("cargo:rustc-link-search=native={}", path.to_str().unwrap());
                }
            }
            Ok(())
        }
        Err(Error::EnvNoPkgConfig(_)) | Err(Error::Command { .. }) => {
            for lib_ in shared_libs.iter() {
                println!("cargo:rustc-link-lib=dylib={}", lib_);
            }
            Ok(())
        }
        Err(err) => Err(err),
    }
}

/// Converts an sRGB color value to a linear sRGB color value (undoes the gamma correction).
///
/// The input and the output are supposed to be in the [0, 1] range.
#[inline]
fn linearize(c: f64) -> f64 {
    if c <= (12.92 * 0.0031308) {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Converts a linear sRGB color value to a normal sRGB color value (applies the gamma correction).
///
/// The input and the output are supposed to be in the [0, 1] range.
#[inline]
fn unlinearize(c: f64) -> f64 {
    if c <= 0.0031308 {
        12.92 * c
    } else {
        1.055 * c.powf(1f64 / 2.4) - 0.055
    }
}

fn print_table<W, F>(w: &mut W, name: &str, f: F, len: u32)
where
    W: Write,
    F: Fn(f64) -> f64,
{
    writeln!(w, "const {}: [u8; {}] = [", name, len).unwrap();

    for i in 0..len {
        let x = f(i as f64 / 255.0);
        let v = (x * 255.0).round() as u8;
        writeln!(w, "    {},", v).unwrap();
    }

    writeln!(w, "];").unwrap();
}

fn generate_srgb_tables() {
    let path = Path::new(&env::var("OUT_DIR").unwrap()).join("srgb-codegen.rs");
    let mut file = BufWriter::new(File::create(&path).unwrap());

    print_table(&mut file, "LINEARIZE", linearize, 256);
    print_table(&mut file, "UNLINEARIZE", unlinearize, 256);
}
