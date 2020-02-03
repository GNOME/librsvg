extern crate assert_cmd;
extern crate predicates;

use assert_cmd::assert::Assert;
use assert_cmd::Command;
use predicates::prelude::*;
use std::path::Path;

struct RsvgConvert {
    cmd: Command,
}

fn location() -> &'static Path {
    match option_env!("LIBRSVG_BUILD_DIR") {
        Some(dir) => Path::new(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap(),
    }
}

impl RsvgConvert {
    fn new() -> Self {
        let path = location().join("rsvg-convert");
        RsvgConvert {
            cmd: Command::new(path),
        }
    }

    fn assert_arg(self: &mut Self, arg: &str) -> Assert {
        self.cmd.arg(arg).assert().success()
    }
}

#[test]
fn help_option() {
    let expected = predicate::str::starts_with("Usage:");
    RsvgConvert::new().assert_arg("-?").stdout(expected.clone());
    RsvgConvert::new().assert_arg("--help").stdout(expected);
}

#[test]
fn version_option() {
    let expected = predicate::str::starts_with("rsvg-convert version ");
    RsvgConvert::new().assert_arg("-v").stdout(expected.clone());
    RsvgConvert::new().assert_arg("--version").stdout(expected);
}
