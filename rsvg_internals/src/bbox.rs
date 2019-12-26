//! Bounding boxes that know their coordinate space.

use cairo;

use crate::rect::{Rect, TransformRect};

#[derive(Debug, Copy, Clone)]
pub struct BoundingBox {
    pub affine: cairo::Matrix,
    pub rect: Option<Rect>,     // without stroke
    pub ink_rect: Option<Rect>, // with stroke
}

impl BoundingBox {
    pub fn new() -> BoundingBox {
        BoundingBox {
            affine: cairo::Matrix::identity(),
            rect: None,
            ink_rect: None,
        }
    }

    pub fn with_affine(self, affine: cairo::Matrix) -> BoundingBox {
        BoundingBox { affine, ..self }
    }

    pub fn with_rect(self, rect: Rect) -> BoundingBox {
        BoundingBox {
            rect: Some(rect),
            ..self
        }
    }

    pub fn with_ink_rect(self, ink_rect: Rect) -> BoundingBox {
        BoundingBox {
            ink_rect: Some(ink_rect),
            ..self
        }
    }

    pub fn clear(mut self) {
        self.rect = None;
        self.ink_rect = None;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine() {
        let r1 = Rect::new(1.0, 2.0, 3.0, 4.0);
        let r2 = Rect::new(1.5, 2.5, 3.5, 4.5);
        let r3 = Rect::new(10.0, 11.0, 12.0, 13.0);
        let affine = cairo::Matrix::new(1.0, 0.0, 0.0, 1.0, 0.5, 0.5);

        let res = combine_rects(None, None, &affine, true);
        assert_eq!(res, None);

        let res = combine_rects(None, None, &affine, false);
        assert_eq!(res, None);

        let res = combine_rects(Some(r1), None, &affine, true);
        assert_eq!(res, Some(r1));

        let res = combine_rects(Some(r1), None, &affine, false);
        assert_eq!(res, Some(r1));

        let res = combine_rects(None, Some(r2), &affine, true);
        assert_eq!(res, Some(Rect::new(2.0, 3.0, 4.0, 5.0)));

        let res = combine_rects(None, Some(r2), &affine, false);
        assert_eq!(res, Some(Rect::new(2.0, 3.0, 4.0, 5.0)));

        let res = combine_rects(Some(r1), Some(r2), &affine, true);
        assert_eq!(res, Some(Rect::new(2.0, 3.0, 3.0, 4.0)));

        let res = combine_rects(Some(r1), Some(r3), &affine, true);
        assert_eq!(res, Some(Rect::default()));

        let res = combine_rects(Some(r1), Some(r2), &affine, false);
        assert_eq!(res, Some(Rect::new(1.0, 2.0, 4.0, 5.0)));
    }
}
