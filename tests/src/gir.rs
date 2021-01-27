extern crate markup5ever_rcdom as rcdom;
extern crate xml5ever;

use std::default::Default;
use std::fs::File;
use std::path::Path;

use rcdom::{Handle, NodeData, RcDom};
use xml5ever::driver::parse_document;
use xml5ever::tendril::TendrilSink;

fn build_dir() -> &'static Path {
    match option_env!("LIBRSVG_BUILD_DIR") {
        Some(dir) => Path::new(dir),
        None => Path::new(env!("CARGO_MANIFEST_DIR")),
    }
}

fn parse(path: &Path) -> std::io::Result<RcDom> {
    let mut file = File::open(path)?;
    parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut file)
}

fn diff(a: &NodeData, b: &NodeData) -> Option<String> {
    match a {
        NodeData::Document if !matches!(b, NodeData::Document) => Some(String::from("root")),
        NodeData::Text { contents: ref c } if !matches!(b, NodeData::Text { contents: ref d } if c.eq(d)) => {
            Some(format!("text: {:?}", c))
        }
        NodeData::Element { name: ref c, .. } if !matches!(b, NodeData::Element { name: ref d, .. } if c.eq(d)) => {
            Some(format!("element: {:?}", c))
        }
        _ => None,
    }
}

fn walk(a: &Handle, b: &Handle) {
    if let Some(diff) = diff(&a.data, &b.data) {
        panic!("files differ: {}", diff);
    }

    for (a, b) in a.children.borrow().iter().zip(b.children.borrow().iter()) {
        walk(a, b);
    }
}

fn escape_default(s: &str) -> String {
    s.chars().flat_map(|c| c.escape_default()).collect()
}

#[test]
fn gobject_introspection_matches_reference() {
    let reference = Path::new("tests/fixtures/gir/Rsvg-2.0-ref.gir");
    let generated = build_dir().join("Rsvg-2.0.gir");

    let a = parse(&generated).unwrap();
    let b = parse(&reference).unwrap();

    walk(&a.document, &b.document);
}
