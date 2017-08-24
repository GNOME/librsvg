extern crate rsvg_sys as ffi;
extern crate glib_sys as glib_ffi;
extern crate gobject_sys as gobject_ffi;

#[macro_use]
extern crate glib;
extern crate cairo;
extern crate gdk_pixbuf;
#[macro_use]
extern crate bitflags;
extern crate libc;

macro_rules! callback_guard {
    () => ()
}

pub use glib::Error;

mod auto;
pub use auto::*;

mod handle;
mod position_data;
mod dimension_data;
pub use position_data::PositionData;
pub use dimension_data::DimensionData;

#[cfg(test)]
#[macro_use]
extern crate imageproc;

#[cfg(test)]
mod tests {
    extern crate image;

    use super::HandleExt;
    use self::image::GenericImage;

    fn get_fixture_path(fixture: &str) -> String {
        return format!("./test-fixtures/{}", fixture);
    }

    #[test]
    fn it_should_be_possible_to_create_new_handle_and_write_manually_to_it() {
        let mut handle = super::Handle::new();

        handle.write(r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="50" height="50"></svg>"#.as_bytes()).unwrap();
        handle.close().unwrap();

        assert_eq!(handle.get_dimensions(), super::DimensionData { width: 50, height: 50, em: 50.0, ex: 50.0 });
        assert_eq!(handle.get_position_sub("#unknownid"), None);
    }

    #[test]
    fn it_should_be_possible_to_load_svg_from_string() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="50" height="50"></svg>"#;
        let handle = super::Handle::new_from_str(svg).unwrap();

        assert_eq!(handle.get_dimensions(), super::DimensionData { width: 50, height: 50, em: 50.0, ex: 50.0 });
        assert_eq!(handle.get_position_sub("#unknownid"), None);
    }

    #[test]
    fn it_should_be_possible_to_load_svg_from_file() {
        let svg_path = get_fixture_path("mysvg.svg");
        let handle = super::Handle::new_from_file(&svg_path).unwrap();

        assert_eq!(handle.get_dimensions(), super::DimensionData { width: 100, height: 100, em: 100.0, ex: 100.0 });
        assert_eq!(handle.get_position_sub("#unknownid"), None);
    }

    #[test]
    fn it_should_be_possible_to_render_to_cairo_context() {
        let svg_path = get_fixture_path("mysvg.svg");
        let expected = image::open(get_fixture_path("mysvg.svg.png")).unwrap();
        let handle = super::Handle::new_from_file(&svg_path).unwrap();
        let dimensions = handle.get_dimensions();
        let surface = super::cairo::ImageSurface::create(super::cairo::Format::ARgb32, dimensions.width, dimensions.height).unwrap();
        let context = super::cairo::Context::new(&surface);
        let mut png_data: Vec<u8> = vec!();

        context.paint_with_alpha(0.0);
        handle.render_cairo(&context);
        surface.write_to_png(&mut png_data).unwrap();

        let result = image::load_from_memory_with_format(&png_data, image::ImageFormat::PNG).unwrap();
        assert_dimensions_match!(result, expected);
        assert_pixels_eq!(result, expected);
    }

    #[test]
    fn it_should_be_possible_to_render_to_gdk_pixbuf_without_throwing() {
        let svg_path = get_fixture_path("mysvg.svg");
        let expected = image::open(get_fixture_path("mysvg.svg.png")).unwrap();
        let handle = super::Handle::new_from_file(&svg_path).unwrap();
        let pixbuf = handle.get_pixbuf().unwrap();
        let pixels = (unsafe { pixbuf.get_pixels() }).to_vec();
        let dimensions = handle.get_dimensions();
        let result = image::ImageBuffer::from_raw(dimensions.width as u32, dimensions.height as u32, pixels)
            .map(|v| image::DynamicImage::ImageRgba8(v))
            .unwrap();

        assert_dimensions_match!(result, expected);
        assert_pixels_eq!(result, expected);
    }

    #[test]
    fn it_should_return_an_error_when_loading_non_existing_file() {
        let handle = super::Handle::new_from_file("unknown.svg");

        assert!(handle.is_err());
    }
}