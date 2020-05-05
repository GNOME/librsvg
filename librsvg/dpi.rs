//! Legacy C API for setting a default DPI (dots per inch = DPI).
//!
//! There are two deprecated functions, `rsvg_set_default_dpi` and
//! `rsvg_set_default_dpi_x_y`, which set global values for the default DPI to be used
//! with `RsvgHandle`.  In turn, `RsvgHandle` assumes that when its own DPI value is set
//! to `0.0` (which is in fact its default), it will fall back to the global DPI.
//!
//! This is clearly not thread-safe, but it is the legacy behavior.
//!
//! This module encapsulates that behavior so that the `rsvg_internals` crate
//! can always have immutable DPI values as intended.

use rsvg_internals;

// This is configurable at runtime
const DEFAULT_DPI_X: f64 = 90.0;
const DEFAULT_DPI_Y: f64 = 90.0;

static mut DPI_X: f64 = DEFAULT_DPI_X;
static mut DPI_Y: f64 = DEFAULT_DPI_Y;

#[derive(Debug, Copy, Clone, Default)]
pub struct Dpi {
    x: f64,
    y: f64,
}

impl Dpi {
    pub fn new(x: f64, y: f64) -> Dpi {
        Dpi { x, y }
    }

    pub fn x(&self) -> f64 {
        if self.x <= 0.0 {
            unsafe { DPI_X }
        } else {
            self.x
        }
    }

    pub fn y(&self) -> f64 {
        if self.y <= 0.0 {
            unsafe { DPI_Y }
        } else {
            self.y
        }
    }
}

impl From<Dpi> for rsvg_internals::Dpi {
    fn from(dpi: Dpi) -> rsvg_internals::Dpi {
        rsvg_internals::Dpi::new(dpi.x(), dpi.y())
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
