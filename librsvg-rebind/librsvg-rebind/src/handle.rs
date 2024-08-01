use glib::{prelude::*, translate::*};

use crate::{Handle, Length, Rectangle};
pub trait HandleExtManual: IsA<Handle> + 'static {
    #[doc(alias = "rsvg_handle_get_intrinsic_dimensions")]
    #[doc(alias = "get_intrinsic_dimensions")]
    fn intrinsic_dimensions(&self) -> (Length, Length, Option<Rectangle>) {
        unsafe {
            let mut out_has_width = std::mem::MaybeUninit::uninit();
            let mut out_width = Length::uninitialized();
            let mut out_has_height = std::mem::MaybeUninit::uninit();
            let mut out_height = Length::uninitialized();
            let mut out_has_viewbox = std::mem::MaybeUninit::uninit();
            let mut out_viewbox = Rectangle::uninitialized();
            ffi::rsvg_handle_get_intrinsic_dimensions(
                self.as_ref().to_glib_none().0,
                out_has_width.as_mut_ptr(),
                out_width.to_glib_none_mut().0,
                out_has_height.as_mut_ptr(),
                out_height.to_glib_none_mut().0,
                out_has_viewbox.as_mut_ptr(),
                out_viewbox.to_glib_none_mut().0,
            );
            (
                out_width,
                out_height,
                (out_has_viewbox.assume_init() != 0).then_some(out_viewbox),
            )
        }
    }
}

impl<O: IsA<Handle>> HandleExtManual for O {}
