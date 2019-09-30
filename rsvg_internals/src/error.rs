use std::error::{self, Error};
use std::fmt;

use cairo;
use cssparser::BasicParseError;
use glib;
use glib::error::ErrorDomain;
use glib::translate::*;
use glib_sys;
use libc;
use markup5ever::LocalName;

use crate::allowed_url::Fragment;
use crate::parsers::ParseError;

/// A simple error which refers to an attribute's value
#[derive(Debug, Clone, PartialEq)]
pub enum ValueErrorKind {
    /// A property with the specified name was not found
    UnknownProperty,

    /// The value could not be parsed
    Parse(ParseError),

    // The value could be parsed, but is invalid
    Value(String),
}

/// A complete error for an attribute and its erroneous value
#[derive(Debug, Clone, PartialEq)]
pub struct NodeError {
    attr: LocalName,
    err: ValueErrorKind,
}

impl NodeError {
    pub fn parse_error(attr: LocalName, error: ParseError) -> NodeError {
        NodeError {
            attr,
            err: ValueErrorKind::Parse(error),
        }
    }

    pub fn value_error(attr: LocalName, description: &str) -> NodeError {
        NodeError {
            attr,
            err: ValueErrorKind::Value(description.to_string()),
        }
    }

    pub fn attribute_error(attr: LocalName, error: ValueErrorKind) -> NodeError {
        NodeError { attr, err: error }
    }
}

impl error::Error for NodeError {
    fn description(&self) -> &str {
        match self.err {
            ValueErrorKind::UnknownProperty => "unknown property",
            ValueErrorKind::Parse(_) => "parse error",
            ValueErrorKind::Value(_) => "invalid attribute value",
        }
    }
}

impl fmt::Display for NodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.err {
            ValueErrorKind::UnknownProperty => write!(f, "unknown property name"),

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

#[derive(Debug, Clone, PartialEq)]
pub enum DefsLookupErrorKind {
    HrefError(HrefError),
    CannotLookupExternalReferences,
    NotFound,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RenderingError {
    Cairo(cairo::Status),
    CircularReference,
    InstancingLimit,
    InvalidId(DefsLookupErrorKind),
    InvalidHref,
    OutOfMemory,
    HandleIsNotLoaded,
}

impl From<cairo::Status> for RenderingError {
    fn from(e: cairo::Status) -> RenderingError {
        assert!(e != cairo::Status::Success);

        RenderingError::Cairo(e)
    }
}

#[derive(Debug)]
pub enum PaintServerError {
    LinkNotFound(Fragment),
    InvalidLinkType(Fragment),
    CircularReference(Fragment),
}

impl error::Error for PaintServerError {
    fn description(&self) -> &str {
        match *self {
            PaintServerError::LinkNotFound(_) => "link to paint server not found",
            PaintServerError::InvalidLinkType(_) => "link is to object of invalid type",
            PaintServerError::CircularReference(_) => "circular reference in link"
        }
    }
}

impl fmt::Display for PaintServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            PaintServerError::LinkNotFound(ref frag) =>
                write!(f, "link to paint server not found: {}", frag),

            PaintServerError::InvalidLinkType(ref frag) =>
                write!(f, "link {} is to object of invalid type", frag),

            PaintServerError::CircularReference(ref frag) =>
                write!(f, "circular reference in link {}", frag),
        }
    }
}

/// Helper for converting `Result<O, E>` into `Result<O, NodeError>`
///
/// A `NodeError` requires a `LocalName` that corresponds to the attribute to which the
/// error refers, plus the actual `ValueErrorKind` that describes the error.  However,
/// parsing functions for attribute value types will want to return their own kind of
/// error, instead of `ValueErrorKind`.  If that particular error type has an `impl
/// From<FooError> for ValueErrorKind`, then this trait helps assign attribute values in
/// `set_atts()` methods as follows:
///
/// ```ignore
/// use error::AttributeResultExt;
///
/// // fn parse_foo(...) -> Result<Foo, FooError>
///
/// // It is assumed that there is an impl From<FooError> for ValueErrorKind
///
/// self.foo = parse_foo(value).attribute(local_name!("foo"))?;
/// ```
///
/// The call to `.attribute(attr)` converts the `Result` from `parse_foo()` into a full
/// `NodeError` with the provided `attr`.
pub trait AttributeResultExt<O, E> {
    fn attribute(self, attr: LocalName) -> Result<O, NodeError>;
}

impl<O, E: Into<ValueErrorKind>> AttributeResultExt<O, E> for Result<O, E> {
    fn attribute(self, attr: LocalName) -> Result<O, NodeError> {
        self.map_err(|e| e.into())
            .map_err(|e| NodeError::attribute_error(attr, e))
    }
}

/// Errors returned when creating an `Href` out of a string
#[derive(Debug, Clone, PartialEq)]
pub enum HrefError {
    /// The href is an invalid URI or has empty components.
    ParseError,

    /// A fragment identifier ("`#foo`") is not allowed here
    ///
    /// For example, the SVG `<image>` element only allows referencing
    /// resources without fragment identifiers like
    /// `xlink:href="foo.png"`.
    FragmentForbidden,

    /// A fragment identifier ("`#foo`") was required but not found.  For example,
    /// the SVG `<use>` element requires one, as in `<use xlink:href="foo.svg#bar">`.
    FragmentRequired,
}

impl From<HrefError> for ValueErrorKind {
    fn from(e: HrefError) -> ValueErrorKind {
        match e {
            HrefError::ParseError => ValueErrorKind::Parse(ParseError::new("url parse error")),
            HrefError::FragmentForbidden => {
                ValueErrorKind::Value("fragment identifier not allowed".to_string())
            }
            HrefError::FragmentRequired => {
                ValueErrorKind::Value("fragment identifier required".to_string())
            }
        }
    }
}

#[derive(Debug, Clone)]
pub enum LoadingError {
    NoDataPassedToParser,
    XmlParseError(String),
    // Could not parse data: URL
    CouldNotCreateXmlParser,
    BadUrl,
    BadDataUrl,
    BadCss,
    Cairo(cairo::Status),
    EmptyData,
    SvgHasNoElements,
    RootElementIsNotSvg,
    Glib(glib::Error),
    Unknown,
}

impl error::Error for LoadingError {
    fn description(&self) -> &str {
        match *self {
            LoadingError::NoDataPassedToParser => "no data passed to parser",
            LoadingError::CouldNotCreateXmlParser => "could not create XML parser",
            LoadingError::XmlParseError(_) => "XML parse error",
            LoadingError::BadUrl => "invalid URL",
            LoadingError::BadDataUrl => "invalid data: URL",
            LoadingError::BadCss => "invalid CSS",
            LoadingError::Cairo(_) => "cairo error",
            LoadingError::EmptyData => "empty data",
            LoadingError::SvgHasNoElements => "SVG has no elements",
            LoadingError::RootElementIsNotSvg => "root element is not <svg>",
            LoadingError::Glib(ref e) => e.description(),
            LoadingError::Unknown => "unknown error",
        }
    }
}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LoadingError::Cairo(status) => write!(f, "cairo error: {:?}", status),
            LoadingError::XmlParseError(ref s) => write!(f, "XML parse error: {}", s),
            LoadingError::NoDataPassedToParser
            | LoadingError::CouldNotCreateXmlParser
            | LoadingError::BadUrl
            | LoadingError::BadDataUrl
            | LoadingError::BadCss
            | LoadingError::EmptyData
            | LoadingError::SvgHasNoElements
            | LoadingError::RootElementIsNotSvg
            | LoadingError::Glib(_)
            | LoadingError::Unknown => write!(f, "{}", self.description()),
        }
    }
}

impl error::Error for RenderingError {
    fn description(&self) -> &str {
        match *self {
            RenderingError::Cairo(_) => "cairo error",
            RenderingError::CircularReference => "circular reference",
            RenderingError::InstancingLimit => "instancing limit",
            RenderingError::InvalidId(_) => "invalid id",
            RenderingError::InvalidHref => "invalid href",
            RenderingError::OutOfMemory => "out of memory",
            RenderingError::HandleIsNotLoaded => "SVG data is not loaded into handle",
        }
    }
}

impl fmt::Display for RenderingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RenderingError::Cairo(ref status) => write!(f, "cairo error: {:?}", status),
            RenderingError::InvalidId(ref id) => write!(f, "invalid id: {:?}", id),
            RenderingError::CircularReference
            | RenderingError::InstancingLimit
            | RenderingError::InvalidHref
            | RenderingError::OutOfMemory
            | RenderingError::HandleIsNotLoaded => write!(f, "{}", self.description()),
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

pub fn set_gerror(err: *mut *mut glib_sys::GError, code: u32, msg: &str) {
    unsafe {
        // this is RSVG_ERROR_FAILED, the only error code available in RsvgError
        assert!(code == 0);

        glib_sys::g_set_error_literal(
            err,
            rsvg_rust_error_quark(),
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
pub const RSVG_ERROR_FAILED: i32 = 0;

impl ErrorDomain for RsvgError {
    fn domain() -> glib::Quark {
        glib::Quark::from_string("rsvg-error-quark")
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

#[no_mangle]
pub extern "C" fn rsvg_rust_error_quark() -> glib_sys::GQuark {
    RsvgError::domain().to_glib()
}
