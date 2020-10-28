use float_cmp::approx_eq;
use librsvg::{CairoRenderer, IntrinsicDimensions, Length, RenderingError};

use crate::c_api::unit_rectangle;

pub trait LegacySize {
    fn legacy_document_size_in_pixels(&self) -> Result<(f64, f64), RenderingError>;
}

impl<'a> LegacySize for CairoRenderer<'a> {
    fn legacy_document_size_in_pixels(&self) -> Result<(f64, f64), RenderingError> {
        let size_from_intrinsic_dimensions = self.intrinsic_size_in_pixels().or_else(|| {
            size_in_pixels_from_percentage_width_and_height(&self.intrinsic_dimensions())
        });

        if let Some(dim) = size_from_intrinsic_dimensions {
            // We have a size directly computed from the <svg> attributes
            Ok(dim)
        } else {
            // Compute the extents of all objects in the SVG
            let (ink_r, _) = self.geometry_for_layer(None, &unit_rectangle())?;
            Ok((ink_r.width, ink_r.height))
        }
    }
}

/// If the width and height are in percentage units, computes a size equal to the
/// `viewBox`'s aspect ratio if it exists, or else returns None.
///
/// For example, a `viewBox="0 0 100 200"` will yield `Some(100.0, 200.0)`.
///
/// Note that this only checks that the width and height are in percentage units, but
/// it actually ignores their values.  This is because at the point this function is
/// called, there is no viewport to embed the SVG document in, so those percentage
/// units cannot be resolved against anything in particular.  The idea is to return
/// some dimensions with the correct aspect ratio.
fn size_in_pixels_from_percentage_width_and_height(
    dim: &IntrinsicDimensions,
) -> Option<(f64, f64)> {
    let IntrinsicDimensions {
        width,
        height,
        vbox,
    } = *dim;

    use librsvg::LengthUnit::*;

    // If both width and height are 100%, just use the vbox size as a pixel size.
    // This gives a size with the correct aspect ratio.

    match (width, height, vbox) {
        (None, None, Some(vbox)) => Some((vbox.width, vbox.height)),

        (
            Some(Length {
                length: w,
                unit: Percent,
            }),
            Some(Length {
                length: h,
                unit: Percent,
            }),
            Some(vbox),
        ) if approx_eq!(f64, w, 1.0) && approx_eq!(f64, h, 1.0) => Some((vbox.width, vbox.height)),

        _ => None,
    }
}
