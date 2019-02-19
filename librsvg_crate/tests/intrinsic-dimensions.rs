extern crate cairo;
extern crate gio;
extern crate glib;
extern crate librsvg;

use gio::MemoryInputStreamExt;
use glib::Cast;

use librsvg::{IntrinsicDimensions, Length, LengthUnit, LoadOptions, SvgHandle};

fn load_svg(input: &'static [u8]) -> SvgHandle {
    let stream = gio::MemoryInputStream::new();
    stream.add_bytes(&glib::Bytes::from_static(input));

    LoadOptions::new()
        .read_stream(&stream.upcast(), None, None)
        .unwrap()
}

#[test]
fn no_intrinsic_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg"/>
"#,
    );

    assert_eq!(
        svg.get_cairo_renderer().get_intrinsic_dimensions(),
        IntrinsicDimensions {
            width: None,
            height: None,
            vbox: None,
        }
    );
}

#[test]
fn has_intrinsic_dimensions() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="10cm" height="20" viewBox="0 0 100 200"/>
"#,
    );

    assert_eq!(
        svg.get_cairo_renderer().get_intrinsic_dimensions(),
        IntrinsicDimensions {
            width: Some(Length::new(10.0, LengthUnit::Cm)),
            height: Some(Length::new(20.0, LengthUnit::Px)),
            vbox: Some(cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 100.0,
                height: 200.0,
            }),
        }
    );
}

#[test]
fn root_geometry_with_percent_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
  <rect x="10" y="20" width="30" height="40"/>
</svg>
"#,
    );

    let renderer = svg.get_cairo_renderer();
    let (ink_r, logical_r) = renderer.get_geometry_for_element(None).unwrap();

    let rect = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}

#[test]
fn element_geometry_with_percent_viewport() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
  <rect id="foo" x="10" y="20" width="30" height="40"/>
</svg>
"#,
    );

    let renderer = svg.get_cairo_renderer();
    let (ink_r, logical_r) = renderer.get_geometry_for_element(Some("#foo")).unwrap();

    let rect = cairo::Rectangle {
        x: 10.0,
        y: 20.0,
        width: 30.0,
        height: 40.0,
    };

    assert_eq!((ink_r, logical_r), (rect, rect));
}
