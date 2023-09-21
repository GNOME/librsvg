use regex::Regex;
use std::env;
use std::fs::File;
use std::io::prelude::*;
use std::io::{BufReader, Write};
use std::path::Path;

fn main() {
    write_version();
}

fn write_version() {
    let mut major = None;
    let mut minor = None;
    let mut micro = None;

    {
        let file = File::open("../configure.ac")
            .expect("builds must take place within the librsvg source tree");

        let major_regex = Regex::new(r"^m4_define\(\[rsvg_major_version\],\[(\d+)\]\)").unwrap();
        let minor_regex = Regex::new(r"^m4_define\(\[rsvg_minor_version\],\[(\d+)\]\)").unwrap();
        let micro_regex = Regex::new(r"^m4_define\(\[rsvg_micro_version\],\[(\d+)\]\)").unwrap();

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

                Err(e) => panic!("could not parse version from configure.ac: {e}"),
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
