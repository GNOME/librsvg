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
    /// Couldn't create an intermediate surface.
    IntermediateSurfaceCreation(cairo::Status),
    /// An intermediate surface has an unsuccessful status.
    BadIntermediateSurfaceStatus(cairo::Status),
}

impl Error for FilterError {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            FilterError::InvalidInput => "invalid value of the `in` attribute",
            FilterError::BadInputSurfaceStatus(_) => "invalid status of the input surface",
            FilterError::IntermediateSurfaceCreation(_) => {
                "couldn't create an intermediate surface"
            }
            FilterError::BadIntermediateSurfaceStatus(_) => {
                "invalid status of an intermediate surface"
            }
        }
    }

    #[inline]
    fn cause(&self) -> Option<&Error> {
        None
    }
}

impl fmt::Display for FilterError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            FilterError::BadInputSurfaceStatus(ref status)
            | FilterError::IntermediateSurfaceCreation(ref status)
            | FilterError::BadIntermediateSurfaceStatus(ref status) => {
                write!(f, "{}: {:?}", self.description(), status)
            }
            _ => write!(f, "{}", self.description()),
        }
    }
}
