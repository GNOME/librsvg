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

/**
 * rsvg_set_default_dpi_x_y:
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi_x_y() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
 */
#[no_mangle]
pub unsafe extern "C" fn rsvg_set_default_dpi_x_y(dpi_x: libc::c_double, dpi_y: libc::c_double) {
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

/**
 * rsvg_set_default_dpi:
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
 */
#[no_mangle]
pub unsafe extern "C" fn rsvg_set_default_dpi(dpi: libc::c_double) {
    if dpi <= 0.0 {
        DPI_X = DEFAULT_DPI_X;
        DPI_Y = DEFAULT_DPI_Y;
    } else {
        DPI_X = dpi;
        DPI_Y = dpi;
    }
}
