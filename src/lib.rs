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