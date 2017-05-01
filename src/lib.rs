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

mod auto;
pub use auto::*;

mod position_data;
mod dimension_data;
pub use position_data::PositionData;
pub use dimension_data::DimensionData;

#[cfg(test)]
mod tests {
    #[test]
    fn it_should_be_possible_to_create_new_handle_and_call_methods() {
        let handle = super::Handle::new();

        assert_eq!(handle.get_dimensions(), super::DimensionData { width: 0, height: 0, em: 0.0, ex: 0.0 });
        assert_eq!(handle.get_position_sub("#unknownid"), None);
    }

    #[test]
    fn it_should_be_possible_to_render_to_cairo_context_without_throwing() {
        let surface = super::cairo::ImageSurface::create(super::cairo::Format::Rgb24, 500, 500);
        let context = super::cairo::Context::new(&surface);
        let handle = super::Handle::new();

        handle.render_cairo(&context);
    }

    #[test]
    fn it_should_be_possible_to_render_to_gdk_pixbuf_without_throwing() {
        let handle = super::Handle::new();

        handle.get_pixbuf();
    }
}