use std::error::Error;
use std::fmt;

use cairo;

/// An enumeration of errors that can occur during filter primitive rendering.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum FilterError {
    /// The filter was passed invalid input (the `in` attribute).
    InvalidInput,
    /// The filter input surface has an unsuccessful status.
    BadInputSurfaceStatus(cairo::Status),
    /// Couldn't create the output surface.
    OutputSurfaceCreation(cairo::Status),
}

impl Error for FilterError {
    #[inline]
    fn description(&self) -> &str {
        match *self {
            FilterError::InvalidInput => "invalid value of the `in` attribute",
            FilterError::BadInputSurfaceStatus(_) => "invalid status of the input surface",
            FilterError::OutputSurfaceCreation(_) => "couldn't create the output surface",
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
            FilterError::BadInputSurfaceStatus(ref status) => {
                write!(f, "{}: {:?}", self.description(), status)
            }
            FilterError::OutputSurfaceCreation(ref status) => {
                write!(f, "{}: {:?}", self.description(), status)
            }
            _ => write!(f, "{}", self.description()),
        }
    }
}
