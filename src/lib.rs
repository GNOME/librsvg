extern crate rsvg_sys as ffi;
extern crate glib_sys as glib_ffi;
extern crate gobject_sys as gobject_ffi;

#[macro_use]
extern crate glib;
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
}