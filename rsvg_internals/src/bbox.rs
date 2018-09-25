use cairo;
use cairo::MatrixTrait;

use rect::RectangleExt;

#[derive(Debug, Copy, Clone)]
pub struct BoundingBox {
    pub affine: cairo::Matrix,
    pub rect: Option<cairo::Rectangle>,     // without stroke
    pub ink_rect: Option<cairo::Rectangle>, // with stroke
}

impl BoundingBox {
    pub fn new(affine: &cairo::Matrix) -> BoundingBox {
        BoundingBox {
            affine: *affine,
            rect: None,
            ink_rect: None,
        }
    }

    pub fn with_rect(self, rect: Option<cairo::Rectangle>) -> BoundingBox {
        BoundingBox { rect, ..self }
    }

    pub fn with_ink_rect(self, ink_rect: Option<cairo::Rectangle>) -> BoundingBox {
        BoundingBox { ink_rect, ..self }
    }

    pub fn with_extents(self, extents: (f64, f64, f64, f64)) -> BoundingBox {
        self.with_rect(rect_from_extents(extents))
    }

    pub fn with_ink_extents(self, extents: (f64, f64, f64, f64)) -> BoundingBox {
        self.with_ink_rect(rect_from_extents(extents))
    }

    fn combine(&mut self, src: &BoundingBox, clip: bool) {
        if src.rect.is_none() && src.ink_rect.is_none() {
            return;
        }

        let mut affine = self.affine;

        // this will panic!() if it's not invertible... should we check on our own?
        affine.invert();
        affine = cairo::Matrix::multiply(&src.affine, &affine);

        self.rect = combine_rects(self.rect, src.rect, &affine, clip);
        self.ink_rect = combine_rects(self.ink_rect, src.ink_rect, &affine, clip);
    }

    pub fn insert(&mut self, src: &BoundingBox) {
        self.combine(src, false);
    }

    pub fn clip(&mut self, src: &BoundingBox) {
        self.combine(src, true);
    }
}

fn rect_from_extents((x1, y1, x2, y2): (f64, f64, f64, f64)) -> Option<cairo::Rectangle> {
    Some(cairo::Rectangle {
        x: x1,
        y: y1,
        width: x2 - x1,
        height: y2 - y1,
    })
}

fn combine_rects(
    r1: Option<cairo::Rectangle>,
    r2: Option<cairo::Rectangle>,
    affine: &cairo::Matrix,
    clip: bool,
) -> Option<cairo::Rectangle> {
    match (r1, r2, clip) {
        (r1, None, _) => r1,
        (None, Some(r2), _) => Some(r2.transform(&affine)),
        (Some(r1), Some(r2), true) => Some(r2.transform(&affine).intersect(&r1)),
        (Some(r1), Some(r2), false) => Some(r2.transform(&affine).union(&r1)),
    }
}
