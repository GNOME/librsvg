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
    write_version();
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

fn write_version() {
    let mut major = None;
    let mut minor = None;
    let mut micro = None;

    {
        let file = File::open("configure.ac")
            .expect("builds must take place within the librsvg source tree");

        let major_regex = Regex::new(r#"^m4_define\(\[rsvg_major_version\],\[(\d+)\]\)"#).unwrap();
        let minor_regex = Regex::new(r#"^m4_define\(\[rsvg_minor_version\],\[(\d+)\]\)"#).unwrap();
        let micro_regex = Regex::new(r#"^m4_define\(\[rsvg_micro_version\],\[(\d+)\]\)"#).unwrap();

        for line in BufReader::new(file).lines() {
            match line {
                Ok(line) => {
                    if let Some(nums) = major_regex.captures(&line) {
                        major = Some(String::from(
                            nums.get(1).expect("major_regex matched once").as_str(),
                        ));
                    } else if let Some(nums) = minor_regex.captures(&line) {
                        minor = Some(String::from(
                            nums.get(1).expect("minor_regex matched once").as_str(),
                        ));
                    } else if let Some(nums) = micro_regex.captures(&line) {
                        micro = Some(String::from(
                            nums.get(1).expect("micro_regex matched once").as_str(),
                        ));
                    }
                }

                Err(e) => panic!("could not parse version from configure.ac: {}", e),
            }
        }
    }

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
            major.expect("major version is set"),
            minor.expect("minor version is set"),
            micro.expect("micro version is set")
        )
        .as_bytes(),
    )
    .expect("write version.rs");
}
