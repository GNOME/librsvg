use cairo;

use crate::rect::{RectangleExt, TransformRect};

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

    pub fn with_rect(self, rect: cairo::Rectangle) -> BoundingBox {
        BoundingBox {
            rect: Some(rect),
            ..self
        }
    }

    pub fn with_ink_rect(self, ink_rect: cairo::Rectangle) -> BoundingBox {
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
    r1: Option<cairo::Rectangle>,
    r2: Option<cairo::Rectangle>,
    affine: &cairo::Matrix,
    clip: bool,
) -> Option<cairo::Rectangle> {
    match (r1, r2, clip) {
        (r1, None, _) => r1,
        (None, Some(r2), _) => Some(affine.transform_rect(&r2)),
        (Some(r1), Some(r2), true) => affine
            .transform_rect(&r2)
            .intersection(&r1)
            .or_else(|| Some(cairo::Rectangle::new(0.0, 0.0, 0.0, 0.0))),
        (Some(r1), Some(r2), false) => Some(affine.transform_rect(&r2).union(&r1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine() {
        let r1 = cairo::Rectangle::new(1.0, 2.0, 3.0, 4.0);
        let r2 = cairo::Rectangle::new(1.5, 2.5, 3.0, 4.0);
        let r3 = cairo::Rectangle::new(10.0, 11.0, 12.0, 13.0);
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
        assert_eq!(res, Some(cairo::Rectangle::new(2.0, 3.0, 3.0, 4.0)));

        let res = combine_rects(None, Some(r2), &affine, false);
        assert_eq!(res, Some(cairo::Rectangle::new(2.0, 3.0, 3.0, 4.0)));

        let res = combine_rects(Some(r1), Some(r2), &affine, true);
        assert_eq!(res, Some(cairo::Rectangle::new(2.0, 3.0, 2.0, 3.0)));

        let res = combine_rects(Some(r1), Some(r3), &affine, true);
        assert_eq!(res, Some(cairo::Rectangle::new(0.0, 0.0, 0.0, 0.0)));

        let res = combine_rects(Some(r1), Some(r2), &affine, false);
        assert_eq!(res, Some(cairo::Rectangle::new(1.0, 2.0, 4.0, 5.0)));
    }
}
