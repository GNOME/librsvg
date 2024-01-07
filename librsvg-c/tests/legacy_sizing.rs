use librsvg_c::sizing::LegacySize;
use rsvg::{CairoRenderer, Loader, LoadingError, SvgHandle};

fn load_svg(input: &'static [u8]) -> Result<SvgHandle, LoadingError> {
    let bytes = glib::Bytes::from_static(input);
    let stream = gio::MemoryInputStream::from_bytes(&bytes);

    Loader::new().read_stream(&stream, None::<&gio::File>, None::<&gio::Cancellable>)
}

#[test]
fn just_viewbox_uses_viewbox_size() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 200"/>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(0.0, 0.0, 100.0, 200.0),
            cairo::Rectangle::new(0.0, 0.0, 100.0, 200.0),
        )
    );
}

#[test]
fn no_intrinsic_size_uses_element_geometries() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(10.0, 20.0, 30.0, 40.0),
            cairo::Rectangle::new(10.0, 20.0, 30.0, 40.0),
        )
    );
}

#[test]
fn hundred_percent_width_height_uses_viewbox() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%" viewBox="0 0 100 200"/>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(0.0, 0.0, 100.0, 200.0),
            cairo::Rectangle::new(0.0, 0.0, 100.0, 200.0),
        )
    );
}

#[test]
fn hundred_percent_width_height_no_viewbox_uses_element_geometries() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="100%">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(10.0, 20.0, 30.0, 40.0),
            cairo::Rectangle::new(10.0, 20.0, 30.0, 40.0),
        )
    );
}

#[test]
fn width_and_viewbox_preserves_aspect_ratio() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="60" viewBox="0 0 30 40">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
    .unwrap();

    // Per the spec, the height property should default to 100%, so the above would end up
    // like <svg width="60" height="100%" viewBox="0 0 30 40">
    //
    // If that were being *rendered* to a viewport, no problem, just use units horizontally
    // and 100% of the viewport vertically.
    //
    // But we are being asked to compute the SVG's natural geometry, so the best we can do
    // is to take the aspect ratio defined by the viewBox and apply it to the width.

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(0.0, 0.0, 60.0, 80.0),
            cairo::Rectangle::new(0.0, 0.0, 60.0, 80.0),
        )
    );
}

#[test]
fn height_and_viewbox_preserves_aspect_ratio() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" height="80" viewBox="0 0 30 40">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
    .unwrap();

    // See the comment above in width_and_viewbox_preserves_aspect_ratio(); this
    // is equivalent but for the height.

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(0.0, 0.0, 60.0, 80.0),
            cairo::Rectangle::new(0.0, 0.0, 60.0, 80.0),
        )
    );
}

#[test]
fn zero_width_vbox() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="80" viewBox="0 0 0 40">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(0.0, 0.0, 0.0, 0.0),
            cairo::Rectangle::new(0.0, 0.0, 0.0, 0.0)
        )
    );
}

#[test]
fn zero_height_vbox() {
    let svg = load_svg(
        br#"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" width="80" viewBox="0 0 30 0">
  <rect x="10" y="20" width="30" height="40" fill="black"/>
</svg>
"#,
    )
    .unwrap();

    assert_eq!(
        CairoRenderer::new(&svg)
            .legacy_layer_geometry(None)
            .unwrap(),
        (
            cairo::Rectangle::new(0.0, 0.0, 0.0, 0.0),
            cairo::Rectangle::new(0.0, 0.0, 0.0, 0.0)
        )
    );
}
