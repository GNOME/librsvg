use glib::translate::FromGlib;

use crate::Unit;

glib::wrapper! {
    #[doc(alias = "RsvgLength")]
    #[derive(Debug)]
    pub struct Length(BoxedInline<ffi::RsvgLength>);
}

impl Length {
    #[inline]
    pub fn length(&self) -> f64 {
        self.inner.length
    }

    pub fn unit(&self) -> Unit {
        unsafe { Unit::from_glib(self.inner.unit) }
    }
}
