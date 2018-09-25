use std::error::Error;
use std::fmt;

use cairo;

/// An enumeration of errors that can occur during filter primitive rendering.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum FilterError {
    /// The filter was passed invalid input (the `in` attribute).
    InvalidInput,
    /// The filter input surface has an unsuccessful status.
    BadInputSurfaceStatus(cairo::Status),
    /// A Cairo error.
    ///
    /// This means that either a failed intermediate surface creation or bad intermediate surface
    /// status.
    CairoError(cairo::Status),
    /// A lighting filter has none or multiple light sources.
    InvalidLightSourceCount,
    /// A lighting filter input surface is too small.
    LightingInputTooSmall,
    /// Child node was in error.
    ChildNodeInError,
}

impl Error for FilterError {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            FilterError::InvalidInput => "invalid value of the `in` attribute",
            FilterError::BadInputSurfaceStatus(_) => "invalid status of the input surface",
            FilterError::CairoError(_) => "Cairo error",
            FilterError::InvalidLightSourceCount => "invalid light source count",
            FilterError::LightingInputTooSmall => {
                "lighting filter input surface is too small (less than 2×2 pixels)"
            }
            FilterError::ChildNodeInError => "child node was in error",
        }
    }

    #[inline]
    fn cause(&self) -> Option<&Error> {
        None
    }
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            FilterError::BadInputSurfaceStatus(ref status)
            | FilterError::CairoError(ref status) => {
                write!(f, "{}: {:?}", self.description(), status)
            }
            _ => write!(f, "{}", self.description()),
        }
    }
}

impl From<cairo::Status> for FilterError {
    #[inline]
    fn from(x: cairo::Status) -> Self {
        FilterError::CairoError(x)
    }
}
