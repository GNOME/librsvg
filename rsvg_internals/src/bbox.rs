use cairo;
use glib_sys;

use cairo::MatrixTrait;
use glib::translate::*;

use float_eq_cairo::ApproxEqCairo;

use rect;

// Keep this in sync with ../../rsvg-private.h:RsvgBbox
#[repr(C)]
#[derive(Copy, Clone)]
pub struct RsvgBbox {
    pub rect: cairo::Rectangle,
    pub affine: cairo::Matrix,
    virgin: glib_sys::gboolean,
}

impl RsvgBbox {
    pub fn new(affine: &cairo::Matrix) -> RsvgBbox {
        RsvgBbox {
            rect: cairo::Rectangle {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },

            affine: *affine,
            virgin: true.to_glib(),
        }
    }

    pub fn is_virgin(&self) -> bool {
        from_glib(self.virgin)
    }

    pub fn is_empty(&self) -> bool {
        from_glib(self.virgin) || self.rect.width.approx_eq_cairo(&0.0)
            || self.rect.height.approx_eq_cairo(&0.0)
    }

    pub fn set_rect(&mut self, r: &cairo::Rectangle) {
        self.rect = *r;
        self.virgin = false.to_glib();
    }

    pub fn insert(&mut self, src: &RsvgBbox) {
        if src.is_virgin() {
            return;
        }

        let mut affine = self.affine;

        // this will panic!() if it's not invertible... should we check on our own?
        affine.invert();
        affine = cairo::Matrix::multiply(&src.affine, &affine);

        let rect = rect::transform(&affine, &src.rect);

        if self.is_virgin() {
            self.set_rect(&rect);
        } else {
            self.rect = rect::union(&rect, &self.rect);
        }
    }

    pub fn clip(&mut self, src: &RsvgBbox) {
        if src.is_virgin() {
            return;
        }

        let mut affine = self.affine;

        affine.invert();
        affine = cairo::Matrix::multiply(&src.affine, &affine);

        let rect = rect::transform(&affine, &src.rect);

        if self.is_virgin() {
            self.set_rect(&rect);
        } else {
            self.rect = rect::intersect(&rect, &self.rect);
        }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_init(raw_bbox: *mut RsvgBbox, raw_matrix: *const cairo::Matrix) {
    assert!(!raw_bbox.is_null());
    assert!(!raw_matrix.is_null());

    let bbox: &mut RsvgBbox = unsafe { &mut (*raw_bbox) };
    let matrix = unsafe { &*raw_matrix };

    *bbox = RsvgBbox::new(matrix);
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_insert(raw_dst: *mut RsvgBbox, raw_src: *const RsvgBbox) {
    assert!(!raw_dst.is_null());
    assert!(!raw_src.is_null());

    let dst: &mut RsvgBbox = unsafe { &mut (*raw_dst) };
    let src: &RsvgBbox = unsafe { &*raw_src };

    dst.insert(src);
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_clip(raw_dst: *mut RsvgBbox, raw_src: *const RsvgBbox) {
    assert!(!raw_dst.is_null());
    assert!(!raw_src.is_null());

    let dst: &mut RsvgBbox = unsafe { &mut (*raw_dst) };
    let src: &RsvgBbox = unsafe { &*raw_src };

    dst.clip(src);
}
