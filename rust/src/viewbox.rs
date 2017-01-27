extern crate cairo;

/* Keep this in sync with rsvg-private.h:RsvgViewBox */
#[repr(C)]
#[derive(Debug, Copy, Clone)]
pub struct RsvgViewBox {
    pub rect:   cairo::Rectangle,
    pub active: bool
}

impl RsvgViewBox {
    pub fn new_inactive () -> RsvgViewBox {
        RsvgViewBox {
            rect: cairo::Rectangle { x: 0.0,
                                     y: 0.0,
                                     width: 0.0,
                                     height: 0.0 },
            active: false
        }
    }
}
