use cairo;
use glib_sys;

use cairo::MatrixTrait;
use glib::translate::*;

use float_eq_cairo::ApproxEqCairo;

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

        let (mut xmin, mut ymin, mut xmax, mut ymax) = if !self.is_virgin() {
            (
                self.rect.x,
                self.rect.y,
                (self.rect.x + self.rect.width),
                (self.rect.y + self.rect.height),
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        let mut affine = self.affine;

        // this will panic!() if it's not invertible... should we check on our own?
        affine.invert();
        affine = cairo::Matrix::multiply(&src.affine, &affine);

        // This is a trick.  We want to transform each of the corners of
        // the rectangle defined by src.rect with the affine
        // transformation, and get the bounding box of all the four
        // resulting points.  The modulus and division accomplish this by
        // running through all the combinations of adding or not adding
        // the width/height to the first point src.rect.(x, y).
        for i in 0..4 {
            let rx: f64 = src.rect.x + src.rect.width * f64::from(i % 2);
            let ry: f64 = src.rect.y + src.rect.height * f64::from(i / 2);
            let x: f64 = affine.xx * rx + affine.xy * ry + affine.x0;
            let y: f64 = affine.yx * rx + affine.yy * ry + affine.y0;

            if self.is_virgin() {
                xmin = x;
                xmax = x;
                ymin = y;
                ymax = y;
                self.virgin = false.to_glib();
            } else {
                if x < xmin {
                    xmin = x;
                }
                if x > xmax {
                    xmax = x;
                }
                if y < ymin {
                    ymin = y;
                }
                if y > ymax {
                    ymax = y;
                }
            }
        }

        self.rect.x = xmin;
        self.rect.y = ymin;
        self.rect.width = xmax - xmin;
        self.rect.height = ymax - ymin;
    }

    pub fn clip(&mut self, src: &RsvgBbox) {
        if src.is_virgin() {
            return;
        }

        let (mut xmin, mut ymin, mut xmax, mut ymax) = if !self.is_virgin() {
            (
                (self.rect.x + self.rect.width),
                (self.rect.y + self.rect.height),
                self.rect.x,
                self.rect.y,
            )
        } else {
            (0.0, 0.0, 0.0, 0.0)
        };

        let mut affine = self.affine;

        affine.invert();
        affine = cairo::Matrix::multiply(&src.affine, &affine);

        // This is a trick.  See rsvg_bbox_insert() for a description of how it works.
        for i in 0..4 {
            let rx: f64 = src.rect.x + src.rect.width * f64::from(i % 2);
            let ry: f64 = src.rect.y + src.rect.height * f64::from(i / 2);
            let x = affine.xx * rx + affine.xy * ry + affine.x0;
            let y = affine.yx * rx + affine.yy * ry + affine.y0;

            if self.is_virgin() {
                xmin = x;
                xmax = x;
                ymin = y;
                ymax = y;
                self.virgin = false.to_glib();
            } else {
                if x < xmin {
                    xmin = x;
                }
                if x > xmax {
                    xmax = x;
                }
                if y < ymin {
                    ymin = y;
                }
                if y > ymax {
                    ymax = y;
                }
            }
        }

        if xmin < self.rect.x {
            xmin = self.rect.x;
        }
        if ymin < self.rect.y {
            ymin = self.rect.y;
        }

        if xmax > self.rect.x + self.rect.width {
            xmax = self.rect.x + self.rect.width;
        }
        if ymax > self.rect.y + self.rect.height {
            ymax = self.rect.y + self.rect.height;
        }

        self.rect.x = xmin;
        self.rect.width = xmax - xmin;
        self.rect.y = ymin;
        self.rect.height = ymax - ymin;
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
