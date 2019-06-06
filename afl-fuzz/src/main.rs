#[macro_use]
extern crate afl;
extern crate cairo;
extern crate glib;
extern crate librsvg;

fn main() {
    fuzz!(|data: &[u8]| {
        let width = 96.;
        let height = 96.;
        let output = "/dev/null";

        let bytes = glib::Bytes::from(data);
        let stream = gio::MemoryInputStream::new_from_bytes(&bytes);
        let handle = librsvg::Loader::new().read_stream(&stream, None, None);
        if let Ok(handle) = handle {
            let renderer = librsvg::CairoRenderer::new(&handle);

            let surface = cairo::svg::File::new(width, height, output);
            let cr = cairo::Context::new(&surface);
            renderer.render_element_to_viewport(
                &cr,
                None,
                &cairo::Rectangle {
                    x: 0.0,
                    y: 0.0,
                    width,
                    height,
                },
            );
        }
    });
}
