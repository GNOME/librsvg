#[derive(Debug)]
pub struct Dpi {
    pub x: f64,
    pub y: f64,
}

pub struct Zoom {
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

    pub fn scale(&self, zoom: Zoom) -> Self {
        Self {
            w: self.w * zoom.x,
            h: self.h * zoom.y,
        }
    }
}
