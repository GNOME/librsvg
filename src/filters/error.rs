use std::fmt;

use crate::error::RenderingError;

/// An enumeration of errors that can occur during filter primitive rendering.
#[derive(Debug, Clone)]
pub enum FilterError {
    /// The units on the filter bounds are invalid
    InvalidUnits,
    /// The filter was passed invalid input (the `in` attribute).
    InvalidInput,
    /// The filter was passed an invalid parameter.
    InvalidParameter(String),
    /// The filter input surface has an unsuccessful status.
    BadInputSurfaceStatus(cairo::Status),
    /// A Cairo error.
    ///
    /// This means that either a failed intermediate surface creation or bad intermediate surface
    /// status.
    CairoError(cairo::Status),
    /// Error from the rendering backend.
    Rendering(RenderingError),
    /// A lighting filter has none or multiple light sources.
    InvalidLightSourceCount,
    /// A lighting filter input surface is too small.
    LightingInputTooSmall,
    /// Child node was in error.
    ChildNodeInError,
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            FilterError::InvalidUnits => write!(
                f,
                "unit identifiers are not allowed with primitiveUnits set to objectBoundingBox"
            ),
            FilterError::InvalidInput => write!(f, "invalid value of the `in` attribute"),
            FilterError::InvalidParameter(ref s) => write!(f, "invalid parameter value: {}", s),
            FilterError::BadInputSurfaceStatus(ref status) => {
                write!(f, "invalid status of the input surface: {}", status)
            }
            FilterError::CairoError(ref status) => write!(f, "Cairo error: {}", status),
            FilterError::Rendering(ref e) => write!(f, "Rendering error: {}", e),
            FilterError::InvalidLightSourceCount => write!(f, "invalid light source count"),
            FilterError::LightingInputTooSmall => write!(
                f,
                "lighting filter input surface is too small (less than 2×2 pixels)"
            ),
            FilterError::ChildNodeInError => write!(f, "child node was in error"),
        }
    }
}

impl From<cairo::Status> for FilterError {
    #[inline]
    fn from(x: cairo::Status) -> Self {
        FilterError::CairoError(x)
    }
}

impl From<RenderingError> for FilterError {
    #[inline]
    fn from(e: RenderingError) -> Self {
        FilterError::Rendering(e)
    }
}
