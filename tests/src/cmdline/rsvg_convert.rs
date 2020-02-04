extern crate assert_cmd;
extern crate predicates;

use crate::cmdline::png_predicate;

use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

struct RsvgConvert {}

impl RsvgConvert {
    fn binary_location() -> &'static Path {
        match option_env!("LIBRSVG_BUILD_DIR") {
            Some(dir) => Path::new(dir),
            None => Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
        }
    }

    fn new() -> Command {
        let path = Self::binary_location().join("rsvg-convert");
        Command::new(path)
    }

    fn new_with_input(input: &Path) -> Command {
        let mut command = RsvgConvert::new();
        match command.pipe_stdin(input) {
            Ok(_) => command,
            Err(e) => panic!("Error opening file '{}': {}", input.display(), e),
        }
    }
}

#[test]
fn empty_input_yields_error() {
    let start = predicate::str::starts_with("Error reading SVG");
    let end = predicate::str::ends_with("Input file is too short");
    RsvgConvert::new()
        .assert()
        .failure()
        .stderr(start.and(end).trim());
}

#[test]
fn reads_from_stdin() {
    let input = Path::new("fixtures/dimensions/521-with-viewbox.svg");
    RsvgConvert::new_with_input(input)
        .assert()
        .success()
        .stdout(png_predicate::has_size(200, 100));
}

#[test]
fn version_option() {
    let out = predicate::str::starts_with("rsvg-convert version ");
    RsvgConvert::new()
        .arg("-v")
        .assert()
        .success()
        .stdout(out.clone());
    RsvgConvert::new()
        .arg("--version")
        .assert()
        .success()
        .stdout(out);
}

#[test]
fn help_option() {
    let out = predicate::str::starts_with("Usage:");
    RsvgConvert::new()
        .arg("-?")
        .assert()
        .success()
        .stdout(out.clone());
    RsvgConvert::new()
        .arg("--help")
        .assert()
        .success()
        .stdout(out);
}
