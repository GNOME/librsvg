//! Tests for the data files from https://github.com/horizon-eda/horizon/
//!
//! Horizon is an app Electronic Design Automation.  It has SVG templates with specially
//! named elements; the app extracts their geometries and renders GUI widgets instead of
//! those elements.  So, it is critical that the geometries get computed accurately.
//!
//! Horizon's build system pre-computes the geometries of the SVG templates' elements, and
//! stores them in JSON files.  You can see the SVGs and the .subs JSON files in the
//! tests/fixtures/horizon in the librsvg source tree.
//!
//! This test file has machinery to load the SVG templates, and the JSON files with the
//! expected geometries.  The tests check that librsvg computes the same geometries every
//! time.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::{collections::BTreeMap};
use std::fs;
use std::path::Path;

// Copy of cairo::Rectangle
//
// Somehow I can't make serde's "remote" work here, in combination with the BTreeMap below...
#[derive(Deserialize, Debug, PartialEq)]
struct Rectangle {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

impl From<cairo::Rectangle> for Rectangle {
    fn from(r: cairo::Rectangle) -> Rectangle {
        Rectangle {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

#[derive(Deserialize)]
struct Geometries(BTreeMap<String, Rectangle>);

fn read_geometries(path: &Path) -> Result<Geometries> {
    let contents = fs::read_to_string(path).context(format!("could not read {:?}", path))?;
    Ok(serde_json::from_str(&contents).context(format!("could not parse JSON from {:?}", path))?)
}

