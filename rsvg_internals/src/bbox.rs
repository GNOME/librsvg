use cairo;
use cairo::MatrixTrait;

use rect::RectangleExt;

// This is only used as *const RsvgBbox or *mut RsvgBbox, as an opaque pointer for C
pub enum RsvgBbox {}

#[derive(Copy, Clone)]
pub struct BoundingBox {
    pub affine: cairo::Matrix,
    pub rect: Option<cairo::Rectangle>,
}

impl BoundingBox {
    pub fn new(affine: &cairo::Matrix) -> BoundingBox {
        BoundingBox {
            affine: *affine,
            rect: None,
        }
    }

    fn combine(&mut self, src: &BoundingBox, clip: bool) {
        if src.rect.is_none() {
            return;
        }

        let mut affine = self.affine;

        // this will panic!() if it's not invertible... should we check on our own?
        affine.invert();
        affine = cairo::Matrix::multiply(&src.affine, &affine);

        let src_rect = src.rect.unwrap().transform(&affine);

        self.rect = match (self.rect, clip) {
            (None, _) => Some(src_rect),
            (Some(r), true) => Some(r.intersect(&src_rect)),
            (Some(r), false) => Some(r.union(&src_rect)),
        };
    }

    pub fn insert(&mut self, src: &BoundingBox) {
        self.combine(src, false);
    }

    pub fn clip(&mut self, src: &BoundingBox) {
        self.combine(src, true);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_new(
    raw_matrix: *const cairo::Matrix,
    raw_rect: *const cairo::Rectangle,
) -> *const RsvgBbox {
    assert!(!raw_matrix.is_null());

    let matrix = unsafe { &*raw_matrix };
    let mut bbox = BoundingBox::new(matrix);

    if !raw_rect.is_null() {
        let rect = unsafe { &*raw_rect };
        bbox.rect = Some(*rect);
    }

    Box::into_raw(Box::new(bbox)) as *const RsvgBbox
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_free(bbox: *mut RsvgBbox) {
    assert!(!bbox.is_null());

    unsafe {
        let bbox = &mut *(bbox as *mut BoundingBox);
        Box::from_raw(bbox);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_clone(bbox: *mut RsvgBbox) -> *const RsvgBbox {
    assert!(!bbox.is_null());

    let bbox = unsafe { &*(bbox as *const BoundingBox) };
    Box::into_raw(Box::new(bbox.clone())) as *const RsvgBbox
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_insert(raw_dst: *mut RsvgBbox, raw_src: *const RsvgBbox) {
    assert!(!raw_dst.is_null());
    assert!(!raw_src.is_null());

    let dst: &mut BoundingBox = unsafe { &mut *(raw_dst as *mut BoundingBox) };
    let src: &BoundingBox = unsafe { &*(raw_src as *const BoundingBox) };

    dst.insert(src);
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_clip(raw_dst: *mut RsvgBbox, raw_src: *const RsvgBbox) {
    assert!(!raw_dst.is_null());
    assert!(!raw_src.is_null());

    let dst: &mut BoundingBox = unsafe { &mut *(raw_dst as *mut BoundingBox) };
    let src: &BoundingBox = unsafe { &*(raw_src as *const BoundingBox) };

    dst.clip(src);
}

#[no_mangle]
pub extern "C" fn rsvg_bbox_get_rect(bbox: *const RsvgBbox, rect: *mut cairo::Rectangle) {
    assert!(!bbox.is_null());

    let bbox: &BoundingBox = unsafe { &*(bbox as *const BoundingBox) };

    if !rect.is_null() {
        if let Some(r) = bbox.rect {
            unsafe { *rect = r };
        }
    }
}
