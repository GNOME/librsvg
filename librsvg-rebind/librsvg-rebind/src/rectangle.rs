use ::glib::translate::*;

glib::wrapper! {
    #[doc(alias = "GtkRectangle")]
    #[derive(Debug)]
    pub struct Rectangle(BoxedInline<ffi::RsvgRectangle>);
}

impl Rectangle {
    #[inline]
    pub fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        assert_initialized_main_thread!();
        unsafe {
            Self::unsafe_from(ffi::RsvgRectangle {
                x,
                y,
                width,
                height,
            })
        }
    }

    #[inline]
    pub fn x(&self) -> f64 {
        self.inner.x
    }

    #[inline]
    pub fn y(&self) -> f64 {
        self.inner.y
    }

    #[inline]
    pub fn width(&self) -> f64 {
        self.inner.width
    }

    #[inline]
    pub fn height(&self) -> f64 {
        self.inner.height
    }
}
