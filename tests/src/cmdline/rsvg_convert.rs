extern crate assert_cmd;
extern crate predicates;

use predicates::prelude::*;
use std::path::Path;

struct RsvgConvert {
    cmd: assert_cmd::Command,
}

impl RsvgConvert {
    fn new() -> Self {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let path = dir.parent().unwrap().join("rsvg-convert");
        RsvgConvert { cmd: assert_cmd::Command::new(path) }
    }
}

#[test]
fn version() {
    let expected = predicate::str::starts_with("rsvg-convert version ");
    RsvgConvert::new().cmd.arg("-v").assert().success().stdout(expected.clone());
    RsvgConvert::new().cmd.arg("--version").assert().success().stdout(expected);
}
