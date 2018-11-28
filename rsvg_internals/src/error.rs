use std::error::{self, Error};
use std::fmt;

use cairo;
use cssparser::BasicParseError;
use glib;
use glib::error::ErrorDomain;
use glib::translate::*;
use glib_sys;
use libc;

use attributes::Attribute;
use parsers::ParseError;

/// A simple error which refers to an attribute's value
#[derive(Debug, Clone, PartialEq)]
pub enum ValueErrorKind {
    /// The value could not be parsed
    Parse(ParseError),

    // The value could be parsed, but is invalid
    Value(String),
}

/// A complete error for an attribute and its erroneous value
#[derive(Debug, Clone, PartialEq)]
pub struct NodeError {
    attr: Attribute,
    err: ValueErrorKind,
}

impl NodeError {
    pub fn parse_error(attr: Attribute, error: ParseError) -> NodeError {
        NodeError {
            attr,
            err: ValueErrorKind::Parse(error),
        }
    }

    pub fn value_error(attr: Attribute, description: &str) -> NodeError {
        NodeError {
            attr,
            err: ValueErrorKind::Value(description.to_string()),
        }
    }

    pub fn attribute_error(attr: Attribute, error: ValueErrorKind) -> NodeError {
        NodeError { attr, err: error }
    }
}

impl error::Error for NodeError {
    fn description(&self) -> &str {
        match self.err {
            ValueErrorKind::Parse(_) => "parse error",
            ValueErrorKind::Value(_) => "invalid attribute value",
        }
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.err {
            ValueErrorKind::Parse(ref n) => write!(
                f,
                "error parsing value for attribute \"{}\": {}",
                self.attr.to_string(),
                n.display
            ),

            ValueErrorKind::Value(ref s) => write!(
                f,
                "invalid value for attribute \"{}\": {}",
                self.attr.to_string(),
                s
            ),
        }
    }
}

impl From<ParseError> for ValueErrorKind {
    fn from(pe: ParseError) -> ValueErrorKind {
        ValueErrorKind::Parse(pe)
    }
}

impl<'a> From<BasicParseError<'a>> for ValueErrorKind {
    fn from(e: BasicParseError<'_>) -> ValueErrorKind {
        ValueErrorKind::from(ParseError::from(e))
    }
}

#[derive(Clone)]
pub enum RenderingError {
    Cairo(cairo::Status),
    CircularReference,
    InstancingLimit,
}

impl From<cairo::Status> for RenderingError {
    fn from(e: cairo::Status) -> RenderingError {
        assert!(e != cairo::Status::Success);

        RenderingError::Cairo(e)
    }
}

#[derive(Debug, Clone)]
pub enum LoadingError {
    // Could not parse data: URL
    BadDataUrl,
    Cairo(cairo::Status),
    EmptyData,
    Glib(glib::Error),
    Unknown,
}

impl error::Error for LoadingError {
    fn description(&self) -> &str {
        match *self {
            LoadingError::BadDataUrl => "invalid data: URL",
            LoadingError::Cairo(_) => "cairo error",
            LoadingError::EmptyData => "empty data",
            LoadingError::Glib(ref e) => e.description(),
            LoadingError::Unknown => "unknown error",
        }
    }
}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LoadingError::Cairo(status) => write!(f, "cairo error: {:?}", status),
            LoadingError::BadDataUrl
            | LoadingError::EmptyData
            | LoadingError::Glib(_)
            | LoadingError::Unknown => write!(f, "{}", self.description()),
        }
    }
}

impl From<cairo::Status> for LoadingError {
    fn from(e: cairo::Status) -> LoadingError {
        assert!(e != cairo::Status::Success);

        LoadingError::Cairo(e)
    }
}

impl From<glib::Error> for LoadingError {
    fn from(e: glib::Error) -> LoadingError {
        LoadingError::Glib(e)
    }
}

extern "C" {
    fn rsvg_error_quark() -> glib_sys::GQuark;
}

pub fn set_gerror(err: *mut *mut glib_sys::GError, code: u32, msg: &str) {
    unsafe {
        // this is RSVG_ERROR_FAILED, the only error code available in RsvgError
        assert!(code == 0);

        glib_sys::g_set_error_literal(
            err,
            rsvg_error_quark(),
            code as libc::c_int,
            msg.to_glib_none().0,
        );
    }
}

#[cfg(test)]
pub fn is_parse_error<T>(r: &Result<T, ValueErrorKind>) -> bool {
    match *r {
        Err(ValueErrorKind::Parse(_)) => true,
        _ => false,
    }
}

#[cfg(test)]
pub fn is_value_error<T>(r: &Result<T, ValueErrorKind>) -> bool {
    match *r {
        Err(ValueErrorKind::Value(_)) => true,
        _ => false,
    }
}

/// Used as a generic error to translate to glib::Error
///
/// This type implements `glib::error::ErrorDomain`, so it can be used
/// to obtain the error code while calling `glib::Error::new()`.  Unfortunately
/// the public librsvg API does not have detailed error codes yet, so we use
/// this single value as the only possible error code to return.
#[derive(Copy, Clone)]
pub struct RsvgError;

// Keep in sync with rsvg.h:RsvgError
const RSVG_ERROR_FAILED: i32 = 0;

impl ErrorDomain for RsvgError {
    fn domain() -> glib::Quark {
        from_glib(unsafe { rsvg_error_quark() })
    }

    fn code(self) -> i32 {
        RSVG_ERROR_FAILED
    }

    fn from(code: i32) -> Option<Self> {
        match code {
            // We don't have enough information from glib error codes
            _ => Some(RsvgError),
        }
    }
}
