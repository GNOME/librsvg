#[derive(Debug)]
pub struct Dpi {
    pub x: f64,
    pub y: f64,
}

pub struct Scale {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug)]
pub struct Size {
    pub w: f64,
    pub h: f64,
}

impl Size {
    pub fn new(w: f64, h: f64) -> Self {
        Self { w, h }
    }

    pub fn scale(&self, scale: Scale) -> Self {
        Self {
            w: self.w * scale.x,
            h: self.h * scale.y,
        }
    }
}
