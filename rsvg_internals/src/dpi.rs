use std::cell::Cell;

// This is configurable at runtime
const DEFAULT_DPI_X: f64 = 90.0;
const DEFAULT_DPI_Y: f64 = 90.0;

static mut DPI_X: f64 = DEFAULT_DPI_X;
static mut DPI_Y: f64 = DEFAULT_DPI_Y;

#[derive(Debug, Clone, Default)]
pub struct Dpi {
    x: Cell<f64>,
    y: Cell<f64>,
}

impl Dpi {
    pub fn x(&self) -> f64 {
        if self.x.get() <= 0.0 {
            unsafe { DPI_X }
        } else {
            self.x.get()
        }
    }

    pub fn set_x(&self, dpi_x: f64) {
        self.x.set(dpi_x)
    }

    pub fn y(&self) -> f64 {
        if self.y.get() <= 0.0 {
            unsafe { DPI_Y }
        } else {
            self.y.get()
        }
    }

    pub fn set_y(&self, dpi_y: f64) {
        self.y.set(dpi_y)
    }
}

#[no_mangle]
pub unsafe extern "C" fn rsvg_rust_set_default_dpi_x_y(dpi_x: f64, dpi_y: f64) {
    if dpi_x <= 0.0 {
        DPI_X = DEFAULT_DPI_X;
    } else {
        DPI_X = dpi_x;
    }

    if dpi_y <= 0.0 {
        DPI_Y = DEFAULT_DPI_Y;
    } else {
        DPI_Y = dpi_y;
    }
}
