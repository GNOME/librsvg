//! Resolution for rendering (dots per inch = DPI).

#[derive(Debug, Copy, Clone)]
pub struct Dpi {
    x: f64,
    y: f64,
}

impl Dpi {
    pub fn new(x: f64, y: f64) -> Dpi {
        Dpi { x, y }
    }

    pub fn x(&self) -> f64 {
        self.x
    }

    pub fn y(&self) -> f64 {
        self.y
    }
}
