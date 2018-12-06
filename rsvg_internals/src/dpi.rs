use std::cell::Cell;

extern "C" {
    fn rsvg_get_default_dpi_y() -> f64;
    fn rsvg_get_default_dpi_x() -> f64;
}

#[derive(Debug, Clone, Default)]
pub struct Dpi {
    x: Cell<f64>,
    y: Cell<f64>,
}

impl Dpi {
    pub fn new(x: f64, y: f64) -> Dpi {
        Dpi {
            x: Cell::new(x),
            y: Cell::new(y),
        }
    }

    pub fn x(&self) -> f64 {
        if self.x.get() <= 0.0 {
            unsafe { rsvg_get_default_dpi_x() }
        } else {
            self.x.get()
        }
    }

    pub fn set_x(&self, dpi_x: f64) {
        self.x.set(dpi_x)
    }

    pub fn y(&self) -> f64 {
        if self.y.get() <= 0.0 {
            unsafe { rsvg_get_default_dpi_y() }
        } else {
            self.y.get()
        }
    }

    pub fn set_y(&self, dpi_y: f64) {
        self.y.set(dpi_y)
    }
}
