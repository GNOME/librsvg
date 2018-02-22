use cairo;
use glib_sys;

use glib::translate::*;
use cairo::MatrixTrait;
use util::*;

/* Keep this in sync with ../../rsvg-private.h:RsvgBbox */
#[repr(C)]
#[derive(Copy, Clone)]
pub struct RsvgBbox {
    pub rect:   cairo::Rectangle,
    pub affine: cairo::Matrix,
    virgin:     glib_sys::gboolean
}

impl RsvgBbox {
    pub fn is_virgin (&self) -> bool {
        from_glib (self.virgin)
    }

    pub fn is_empty(&self) -> bool {
        from_glib(self.virgin) || double_equals(self.rect.width, 0.0) || double_equals(self.rect.height, 0.0)
    }
}

#[no_mangle]
pub extern fn rsvg_bbox_init (raw_bbox: *mut RsvgBbox, raw_matrix: *const cairo::Matrix) {
    assert! (!raw_bbox.is_null ());
    assert! (!raw_matrix.is_null ());

    let bbox: &mut RsvgBbox = unsafe { &mut (*raw_bbox) };

    bbox.virgin = true.to_glib ();
    bbox.affine = unsafe { *raw_matrix };
}

#[no_mangle]
pub extern fn rsvg_bbox_insert (raw_dst: *mut RsvgBbox, raw_src: *const RsvgBbox) {
    assert! (!raw_dst.is_null ());
    assert! (!raw_src.is_null ());

    let dst: &mut RsvgBbox = unsafe { &mut (*raw_dst) };
    let src: &RsvgBbox = unsafe { &*raw_src };

    if src.is_virgin () {
        return;
    }

    let (mut xmin, mut ymin, mut xmax, mut ymax) = if !dst.is_virgin () {
        (dst.rect.x,
         dst.rect.y,
         (dst.rect.x + dst.rect.width),
         (dst.rect.y + dst.rect.height))
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    let mut affine = dst.affine;

    affine.invert (); // this will panic!() if it's not invertible... should we check on our own?
    affine = cairo::Matrix::multiply (&src.affine, &affine);

    /* This is a trick.  We want to transform each of the corners of
     * the rectangle defined by src.rect with the affine
     * transformation, and get the bounding box of all the four
     * resulting points.  The modulus and division accomplish this by
     * running through all the combinations of adding or not adding
     * the width/height to the first point src.rect.(x, y).
     */
    for i in 0..4 {
        let rx: f64 = src.rect.x + src.rect.width * f64::from(i % 2);
        let ry: f64 = src.rect.y + src.rect.height * f64::from(i / 2);
        let x: f64  = affine.xx * rx + affine.xy * ry + affine.x0;
        let y: f64  = affine.yx * rx + affine.yy * ry + affine.y0;

        if dst.is_virgin () {
            xmin = x;
            xmax = x;
            ymin = y;
            ymax = y;
            dst.virgin = false.to_glib ();
        } else {
            if x < xmin { xmin = x; }
            if x > xmax { xmax = x; }
            if y < ymin { ymin = y; }
            if y > ymax { ymax = y; }
        }
    }

    dst.rect.x = xmin;
    dst.rect.y = ymin;
    dst.rect.width = xmax - xmin;
    dst.rect.height = ymax - ymin;
}

#[no_mangle]
pub extern fn rsvg_bbox_clip (raw_dst: *mut RsvgBbox, raw_src: *const RsvgBbox) {
    assert! (!raw_dst.is_null ());
    assert! (!raw_src.is_null ());

    let dst: &mut RsvgBbox = unsafe { &mut (*raw_dst) };
    let src: &RsvgBbox = unsafe { &*raw_src };

    if src.is_virgin () {
        return;
    }

    let (mut xmin, mut ymin, mut xmax, mut ymax) = if !dst.is_virgin () {
        ((dst.rect.x + dst.rect.width),
         (dst.rect.y + dst.rect.height),
         dst.rect.x,
         dst.rect.y)
    } else {
        (0.0, 0.0, 0.0, 0.0)
    };

    let mut affine = dst.affine;

    affine.invert ();
    affine = cairo::Matrix::multiply (&src.affine, &affine);

    /* This is a trick.  See rsvg_bbox_insert() for a description of how it works. */
    for i in 0..4 {
        let rx: f64 = src.rect.x + src.rect.width * f64::from(i % 2);
        let ry: f64 = src.rect.y + src.rect.height * f64::from(i / 2);
        let x = affine.xx * rx + affine.xy * ry + affine.x0;
        let y = affine.yx * rx + affine.yy * ry + affine.y0;

        if dst.is_virgin () {
            xmin = x;
            xmax = x;
            ymin = y;
            ymax = y;
            dst.virgin = false.to_glib ();
        } else {
            if x < xmin { xmin = x; }
            if x > xmax { xmax = x; }
            if y < ymin { ymin = y; }
            if y > ymax { ymax = y; }
        }
    }

    if xmin < dst.rect.x { xmin = dst.rect.x; }
    if ymin < dst.rect.y { ymin = dst.rect.y; }

    if xmax > dst.rect.x + dst.rect.width { xmax = dst.rect.x + dst.rect.width; }
    if ymax > dst.rect.y + dst.rect.height { ymax = dst.rect.y + dst.rect.height; }

    dst.rect.x = xmin;
    dst.rect.width = xmax - xmin;
    dst.rect.y = ymin;
    dst.rect.height = ymax - ymin;
}
