//! Error types.

use std::error;
use std::fmt;

use cssparser::{BasicParseError, BasicParseErrorKind, ParseErrorKind, ToCss};
use markup5ever::QualName;

use crate::allowed_url::Fragment;
use crate::node::Node;

/// A short-lived error.
///
/// The lifetime of the error is the same as the `cssparser::ParserInput` that
/// was used to create a `cssparser::Parser`.  That is, it is the lifetime of
/// the string data that is being parsed.
///
/// The code flow will sometimes require preserving this error as a long-lived struct;
/// see the `impl<'i, O> AttributeResultExt<O> for Result<O, ParseError<'i>>` for that
/// purpose.
pub type ParseError<'i> = cssparser::ParseError<'i, ValueErrorKind>;

/// A simple error which refers to an attribute's value
#[derive(Debug, Clone, PartialEq)]
pub enum ValueErrorKind {
    /// A property with the specified name was not found
    UnknownProperty,

    /// The value could not be parsed
    Parse(String),

    // The value could be parsed, but is invalid
    Value(String),
}

impl ValueErrorKind {
    pub fn parse_error(s: &str) -> ValueErrorKind {
        ValueErrorKind::Parse(s.to_string())
    }

    pub fn value_error(s: &str) -> ValueErrorKind {
        ValueErrorKind::Value(s.to_string())
    }
}

impl fmt::Display for ValueErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ValueErrorKind::UnknownProperty => write!(f, "unknown property name"),

            ValueErrorKind::Parse(ref s) => write!(f, "parse error: {}", s),

            ValueErrorKind::Value(ref s) => write!(f, "invalid value: {}", s),
        }
    }
}

impl<'a> From<BasicParseError<'a>> for ValueErrorKind {
    fn from(e: BasicParseError<'_>) -> ValueErrorKind {
        let BasicParseError { kind, .. } = e;

        let msg = match kind {
            BasicParseErrorKind::UnexpectedToken(_) => "unexpected token",
            BasicParseErrorKind::EndOfInput => "unexpected end of input",
            BasicParseErrorKind::AtRuleInvalid(_) => "invalid @-rule",
            BasicParseErrorKind::AtRuleBodyInvalid => "invalid @-rule body",
            BasicParseErrorKind::QualifiedRuleInvalid => "invalid qualified rule",
        };

        ValueErrorKind::parse_error(msg)
    }
}

/// A complete error for an attribute and its erroneous value
#[derive(Debug, Clone, PartialEq)]
pub struct ElementError {
    pub attr: QualName,
    pub err: ValueErrorKind,
}

impl error::Error for ElementError {
    fn description(&self) -> &str {
        match self.err {
            ValueErrorKind::UnknownProperty => "unknown property",
            ValueErrorKind::Parse(_) => "parse error",
            ValueErrorKind::Value(_) => "invalid attribute value",
        }
    }
}

impl fmt::Display for ElementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.attr.expanded(), self.err)
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

impl error::Error for RenderingError {}

impl fmt::Display for RenderingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RenderingError::CircularReference => write!(f, "circular reference"),
            RenderingError::InstancingLimit => write!(f, "instancing limit"),
            RenderingError::InvalidHref => write!(f, "invalid href"),
            RenderingError::OutOfMemory => write!(f, "out of memory"),
            RenderingError::HandleIsNotLoaded => write!(f, "SVG data is not loaded into handle"),
            RenderingError::Cairo(ref status) => write!(f, "cairo error: {:?}", status),
            RenderingError::InvalidId(ref id) => write!(f, "invalid id: {:?}", id),
        }
    }
}

impl From<cairo::Status> for RenderingError {
    fn from(e: cairo::Status) -> RenderingError {
        assert!(e != cairo::Status::Success);

        RenderingError::Cairo(e)
    }
}

pub enum AcquireError {
    LinkNotFound(Fragment),
    InvalidLinkType(Fragment),
    CircularReference(Node),
    MaxReferencesExceeded,
}

impl fmt::Display for AcquireError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            AcquireError::LinkNotFound(ref frag) => write!(f, "link not found: {}", frag),

            AcquireError::InvalidLinkType(ref frag) => {
                write!(f, "link {} is to object of invalid type", frag)
            }

            AcquireError::CircularReference(ref node) => {
                write!(f, "circular reference in node {}", node)
            }

            AcquireError::MaxReferencesExceeded => {
                write!(f, "maximum number of references exceeded")
            }
        }
    }
}

/// Helper for converting `Result<O, E>` into `Result<O, ElementError>`
///
/// A `ElementError` requires a `QualName` that corresponds to the attribute to which the
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
/// `ElementError` with the provided `attr`.
pub trait AttributeResultExt<O> {
    fn attribute(self, attr: QualName) -> Result<O, ElementError>;
}

impl<O, E: Into<ValueErrorKind>> AttributeResultExt<O> for Result<O, E> {
    fn attribute(self, attr: QualName) -> Result<O, ElementError> {
        self.map_err(|e| e.into())
            .map_err(|err| ElementError { attr, err })
    }
}

/// Turns a short-lived `ParseError` into a long-lived `ElementError`
impl<'i, O> AttributeResultExt<O> for Result<O, ParseError<'i>> {
    fn attribute(self, attr: QualName) -> Result<O, ElementError> {
        self.map_err(|e| {
            // FIXME: eventually, here we'll want to preserve the location information

            let ParseError {
                kind,
                location: _location,
            } = e;

            match kind {
                ParseErrorKind::Basic(BasicParseErrorKind::UnexpectedToken(tok)) => {
                    let mut s = String::from("unexpected token '");
                    tok.to_css(&mut s).unwrap(); // FIXME: what do we do with a fmt::Error?
                    s.push_str("'");

                    ElementError {
                        attr,
                        err: ValueErrorKind::Parse(s),
                    }
                }

                ParseErrorKind::Basic(BasicParseErrorKind::EndOfInput) => ElementError {
                    attr,
                    err: ValueErrorKind::parse_error("unexpected end of input"),
                },

                ParseErrorKind::Basic(_) => {
                    unreachable!("attribute parsers should not return errors for CSS rules")
                }

                ParseErrorKind::Custom(err) => ElementError { attr, err },
            }
        })
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
            HrefError::ParseError => ValueErrorKind::parse_error("url parse error"),
            HrefError::FragmentForbidden => {
                ValueErrorKind::value_error("fragment identifier not allowed")
            }
            HrefError::FragmentRequired => {
                ValueErrorKind::value_error("fragment identifier required")
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
    BadStylesheet,
    BadCss,
    Cairo(cairo::Status),
    EmptyData,
    SvgHasNoElements,
    RootElementIsNotSvg,
    Glib(glib::Error),
    Unknown,
}

impl error::Error for LoadingError {}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LoadingError::NoDataPassedToParser => write!(f, "no data passed to parser"),
            LoadingError::XmlParseError(ref s) => write!(f, "XML parse error: {}", s),
            LoadingError::CouldNotCreateXmlParser => write!(f, "could not create XML parser"),
            LoadingError::BadUrl => write!(f, "invalid URL"),
            LoadingError::BadDataUrl => write!(f, "invalid data: URL"),
            LoadingError::BadStylesheet => write!(f, "invalid stylesheet"),
            LoadingError::BadCss => write!(f, "invalid CSS"),
            LoadingError::Cairo(status) => write!(f, "cairo error: {:?}", status),
            LoadingError::EmptyData => write!(f, "empty data"),
            LoadingError::SvgHasNoElements => write!(f, "SVG has no elements"),
            LoadingError::RootElementIsNotSvg => write!(f, "root element is not <svg>"),
            LoadingError::Glib(ref e) => e.fmt(f),
            LoadingError::Unknown => write!(f, "unknown error"),
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
