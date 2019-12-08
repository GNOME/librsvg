use cairo;

use crate::rect::{Rect, TransformRect};

#[derive(Debug, Copy, Clone)]
pub struct BoundingBox {
    pub affine: cairo::Matrix,
    pub rect: Option<Rect>,     // without stroke
    pub ink_rect: Option<Rect>, // with stroke
}

impl BoundingBox {
    pub fn new(affine: &cairo::Matrix) -> BoundingBox {
        BoundingBox {
            affine: *affine,
            rect: None,
            ink_rect: None,
        }
    }

    pub fn with_rect(self, rect: Option<Rect>) -> BoundingBox {
        BoundingBox { rect, ..self }
    }

    pub fn with_ink_rect(self, ink_rect: Option<Rect>) -> BoundingBox {
        BoundingBox { ink_rect, ..self }
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

fn combine_rects(
    r1: Option<Rect>,
    r2: Option<Rect>,
    affine: &cairo::Matrix,
    clip: bool,
) -> Option<Rect> {
    match (r1, r2, clip) {
        (r1, None, _) => r1,
        (None, Some(r2), _) => Some(affine.transform_rect(&r2)),
        (Some(r1), Some(r2), true) => affine
            .transform_rect(&r2)
            .intersection(&r1)
            .or_else(|| Some(Rect::default())),
        (Some(r1), Some(r2), false) => Some(affine.transform_rect(&r2).union(&r1)),
    }
}
