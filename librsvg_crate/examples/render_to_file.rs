fn main() {
    let width = 96.0;
    let height = 96.0;
    let output = "output.svg";

    let bytes = glib::Bytes::from_static(include_bytes!("org.gnome.Epiphany.svg"));
    let stream = gio::MemoryInputStream::new_from_bytes(&bytes);
    let handle = librsvg::Loader::new()
        .read_stream(&stream, None, None)
        .unwrap();
    let renderer = librsvg::CairoRenderer::new(&handle);

    let surface = cairo::SvgSurface::new(width, height, output);
    let cr = cairo::Context::new(&surface);
    renderer
        .render_to_viewport(
            &cr,
            &cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width,
                height,
            },
        )
        .unwrap();
}
