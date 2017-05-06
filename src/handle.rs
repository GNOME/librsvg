use ffi;
use std::ptr;

use glib::Error;
use glib::translate::*;
use auto::Handle;

impl Handle {
    pub fn new_from_str(data: &str) -> Result<Handle, Error> {
        unsafe {
            let mut error = ptr::null_mut();
            let handle = ffi::rsvg_handle_new_from_data(data.as_ptr() as *mut _, data.len() as _, &mut error);
            if error.is_null() { Ok(from_glib_full(handle)) } else { Err(from_glib_full(error)) }
        }
    }

    pub fn write(&mut self, data: &str) -> Result<(), Error> {
        unsafe {
            let mut error = ptr::null_mut();
            ffi::rsvg_handle_write(self.to_glib_none().0, data.as_ptr() as *mut _, data.len() as _, &mut error);
            if error.is_null() { Ok(()) } else { Err(from_glib_full(error)) }
        }
    }
}