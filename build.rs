use regex::Regex;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;
use std::process;

fn main() {
    if let Err(e) = system_deps::Config::new().probe() {
        eprintln!("{}", e);
        process::exit(1);
    }

    generate_srgb_tables();

    let version = read_version_from_meson_build("meson.build").expect("Could not find version in meson.build");
    write_version_rs(&version);
    write_rsvg_version_h(&version);
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

struct Version {
    major: String,
    minor: String,
    micro: String,
}

fn read_version_from_meson_build(filename: &str) -> Option<Version> {
    {
        let file = File::open(filename)
            .expect("builds must take place within the librsvg source tree");

        // This function reads one of the build configuration files (meson.build) and scans
        // it for the package's version number.
        //
        // The start of meson.build should contain this:
        //
        //   project('librsvg', 'rust', 'c', version: '2.55.90', meson_version: '>= 0.63')
        //
        // This regex looks for the "version" line.

        let regex = Regex::new(r#"^project\(.*version: '(\d+\.\d+\.\d+)'"#).unwrap();

        for line in BufReader::new(file).lines() {
            match line {
                Ok(line) => {
                    if let Some(caps) = regex.captures(&line) {
                        let version_str = &caps[1];
                        let mut components = version_str.split(".");
                        let major = components.next().unwrap().to_string();
                        let minor = components.next().unwrap().to_string();
                        let micro = components.next().unwrap().to_string();
                        return Some(Version {
                            major,
                            minor,
                            micro,
                        });
                    }
                }

                Err(e) => panic!("could not read line from {}: {}", filename, e),
            }
        }
    }

    None
}

fn write_version_rs(version: &Version) {
    let output = Path::new(&env::var("OUT_DIR").unwrap()).join("version.rs");
    let mut file = File::create(output).expect("open version.rs for writing");
    file.write_all(
        format!(
            r#"
use std::os::raw::c_uint;

#[no_mangle]
pub static rsvg_major_version: c_uint = {};

#[no_mangle]
pub static rsvg_minor_version: c_uint = {};

#[no_mangle]
pub static rsvg_micro_version: c_uint = {};
"#,
            version.major,
            version.minor,
            version.micro,
        )
        .as_bytes(),
    )
    .expect("write version.rs");
}

fn write_rsvg_version_h(version: &Version) {
    let output = Path::new(&env::var("OUT_DIR").unwrap()).join("rsvg-version.h");
    let mut file = File::create(output).expect("open rsvg-version.h for writing");
    file.write_all(
        format!(
r##"#if !defined (__RSVG_RSVG_H_INSIDE__) && !defined (RSVG_COMPILATION)
#warning "Including <librsvg/rsvg-version.h> directly is deprecated."
#endif

#ifndef RSVG_VERSION_H
#define RSVG_VERSION_H

#define LIBRSVG_MAJOR_VERSION ({major})
#define LIBRSVG_MINOR_VERSION ({minor})
#define LIBRSVG_MICRO_VERSION ({micro})
#define LIBRSVG_VERSION "{major}.{minor}.{micro}"

#endif
"##,
            major = version.major,
            minor = version.minor,
            micro = version.micro,
        ).as_bytes()
    )
    .expect("write rsvg-version.h");
}
