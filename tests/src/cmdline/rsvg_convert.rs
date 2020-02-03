extern crate assert_cmd;
extern crate predicates;

use predicates::prelude::*;
use std::path::Path;

struct RsvgConvert {
    cmd: assert_cmd::Command,
}

fn location() -> &'static Path {
    match option_env!("LIBRSVG_BUILD_DIR") {
        Some(dir) => Path::new(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap()
    }
}

impl RsvgConvert {
    fn new() -> Self {
        let path = location().join("rsvg-convert");
        println!("{:?}", path);
        RsvgConvert { cmd: assert_cmd::Command::new(path) }
    }
}

#[test]
fn version() {
    let expected = predicate::str::starts_with("rsvg-convert version ");
    RsvgConvert::new().cmd.arg("-v").assert().success().stdout(expected.clone());
    RsvgConvert::new().cmd.arg("--version").assert().success().stdout(expected);
}
