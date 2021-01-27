extern crate markup5ever;
extern crate markup5ever_rcdom as rcdom;
extern crate xml5ever;

use std::cell::RefCell;
use std::default::Default;
use std::fs::File;
use std::path::Path;

use markup5ever::Attribute;
use markup5ever::QualName;
use rcdom::{Handle, NodeData, RcDom};
use xml5ever::driver::parse_document;
use xml5ever::tendril::{StrTendril, TendrilSink};

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

fn format_text(text: &RefCell<StrTendril>) -> String {
    format!("text: {}", escape_default(&text.borrow()))
}

fn format_element(name: &QualName, attrs: &RefCell<Vec<Attribute>>) -> String {
    let mut str = String::from(format!("<{}", name.local));
    for attr in attrs.borrow().iter() {
        str.push_str(&format!(" {}=\"{}\"", attr.name.local, attr.value));
    }
    str.push_str(">");
    str
}

fn equal(a: &RefCell<Vec<Attribute>>, b: &RefCell<Vec<Attribute>>, ignore_name: &str) -> bool {
    let ignore = |&attr: &&Attribute| -> bool { !attr.name.local.eq(ignore_name) };
    a.borrow()
        .iter()
        .filter(ignore)
        .zip(b.borrow().iter().filter(ignore))
        .all(|(a, b)| a.eq(b))
}

fn diff(a: &NodeData, b: &NodeData) -> Option<String> {
    match a {
        NodeData::Document if !matches!(b, NodeData::Document) => Some(String::from("root")),
        NodeData::Text { contents: ref a } if !matches!(b, NodeData::Text { contents: ref b } if a.eq(&b)) => {
            Some(format_text(&a))
        }
        NodeData::Element {
            name: ref a,
            attrs: ref a_attrs,
            ..
        } if !matches!(b, NodeData::Element { name: ref b, attrs: ref b_attrs, .. } if a.eq(&b) && equal(&a_attrs, &b_attrs, "line")) => {
            Some(format_element(&a, &a_attrs))
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
