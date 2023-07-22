//! Error types.

use std::error;
use std::fmt;

use cssparser::{BasicParseError, BasicParseErrorKind, ParseErrorKind, ToCss};
use markup5ever::QualName;

use crate::document::NodeId;
use crate::io::IoError;
use crate::limits;
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
#[derive(Debug, Clone)]
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
#[derive(Debug, Clone)]
pub struct ElementError {
    pub attr: QualName,
    pub err: ValueErrorKind,
}

impl fmt::Display for ElementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.attr.expanded(), self.err)
    }
}

/// Errors returned when looking up a resource by URL reference.
#[derive(Debug, Clone)]
pub enum DefsLookupErrorKind {
    /// Error when parsing the id to lookup.
    InvalidId,

    /// Used when the public API tries to look up an external URL, which is not allowed.
    ///
    /// This catches the case where a public API wants to be misused to access an external
    /// resource.  For example, `SvgHandle.has_sub("https://evil.com/phone_home#element_id")` will
    /// fail with this error.
    CannotLookupExternalReferences,

    /// For internal use only.
    ///
    // FIXME: this is returned internally from Handle.lookup_node(), and gets translated
    // to Ok(false).  Don't expose this internal code in the public API.
    NotFound,
}

impl fmt::Display for DefsLookupErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            DefsLookupErrorKind::InvalidId => write!(f, "invalid id"),
            DefsLookupErrorKind::CannotLookupExternalReferences => {
                write!(f, "cannot lookup references to elements in external files")
            }
            DefsLookupErrorKind::NotFound => write!(f, "not found"),
        }
    }
}

/// Errors that can happen while rendering or measuring an SVG document.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum RenderingError {
    /// An error from the rendering backend.
    Rendering(String),

    /// A particular implementation-defined limit was exceeded.
    LimitExceeded(ImplementationLimit),

    /// Tried to reference an SVG element that does not exist.
    IdNotFound,

    /// Tried to reference an SVG element from a fragment identifier that is incorrect.
    InvalidId(String),

    /// Not enough memory was available for rendering.
    OutOfMemory(String),
}

impl From<DefsLookupErrorKind> for RenderingError {
    fn from(e: DefsLookupErrorKind) -> RenderingError {
        match e {
            DefsLookupErrorKind::NotFound => RenderingError::IdNotFound,
            _ => RenderingError::InvalidId(format!("{}", e)),
        }
    }
}

impl error::Error for RenderingError {}

impl fmt::Display for RenderingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RenderingError::Rendering(ref s) => write!(f, "rendering error: {}", s),
            RenderingError::LimitExceeded(ref l) => write!(f, "{}", l),
            RenderingError::IdNotFound => write!(f, "element id not found"),
            RenderingError::InvalidId(ref s) => write!(f, "invalid id: {:?}", s),
            RenderingError::OutOfMemory(ref s) => write!(f, "out of memory: {}", s),
        }
    }
}

impl From<cairo::Error> for RenderingError {
    fn from(e: cairo::Error) -> RenderingError {
        RenderingError::Rendering(format!("{:?}", e))
    }
}

/// Errors from [`crate::document::AcquiredNodes`].
pub enum AcquireError {
    /// An element with the specified id was not found.
    LinkNotFound(NodeId),

    InvalidLinkType(NodeId),

    /// A circular reference was detected; non-fatal error.
    ///
    /// Callers are expected to treat the offending element as invalid, for example
    /// if a graphic element uses a pattern fill, but the pattern in turn includes
    /// another graphic element that references the same pattern.
    ///
    /// ```xml
    /// <pattern id="foo">
    ///   <rect width="1" height="1" fill="url(#foo)"/>
    /// </pattern>
    /// ```
    CircularReference(Node),

    /// Too many referenced objects were resolved; fatal error.
    ///
    /// Callers are expected to exit as early as possible and return an error to
    /// the public API.  See [`ImplementationLimit::TooManyReferencedElements`] for details.
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
/// ```
/// # use librsvg::doctest_only::AttributeResultExt;
/// # use librsvg::doctest_only::ValueErrorKind;
/// # use librsvg::doctest_only::ElementError;
/// # use markup5ever::{QualName, Prefix, Namespace, LocalName};
/// # type FooError = ValueErrorKind;
/// fn parse_foo(value: &str) -> Result<(), FooError>
/// # { Err(ValueErrorKind::value_error("test")) }
///
/// // It is assumed that there is an impl From<FooError> for ValueErrorKind
/// # let attr = QualName::new(
/// #     Some(Prefix::from("")),
/// #     Namespace::from(""),
/// #     LocalName::from(""),
/// # );
/// let result = parse_foo("value").attribute(attr);
/// assert!(result.is_err());
/// # Ok::<(), ElementError>(())
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
                    s.push('\'');

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

/// Errors returned when resolving an URL
#[derive(Debug, Clone)]
pub enum AllowedUrlError {
    /// parsing error from `Url::parse()`
    UrlParseError(url::ParseError),

    /// A base file/uri was not set
    BaseRequired,

    /// Cannot reference a file with a different URI scheme from the base file
    DifferentUriSchemes,

    /// Some scheme we don't allow loading
    DisallowedScheme,

    /// The requested file is not in the same directory as the base file,
    /// or in one directory below the base file.
    NotSiblingOrChildOfBaseFile,

    /// Loaded file:// URLs cannot have a query part, e.g. `file:///foo?blah`
    NoQueriesAllowed,

    /// URLs may not have fragment identifiers at this stage
    NoFragmentIdentifierAllowed,

    /// Error when obtaining the file path or the base file path
    InvalidPath,

    /// The base file cannot be the root of the file system
    BaseIsRoot,

    /// Error when canonicalizing either the file path or the base file path
    CanonicalizationError,
}

impl fmt::Display for AllowedUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use AllowedUrlError::*;
        match *self {
            UrlParseError(e) => write!(f, "URL parse error: {}", e),
            BaseRequired => write!(f, "base required"),
            DifferentUriSchemes => write!(f, "different URI schemes"),
            DisallowedScheme => write!(f, "disallowed scheme"),
            NotSiblingOrChildOfBaseFile => write!(f, "not sibling or child of base file"),
            NoQueriesAllowed => write!(f, "no queries allowed"),
            NoFragmentIdentifierAllowed => write!(f, "no fragment identifier allowed"),
            InvalidPath => write!(f, "invalid path"),
            BaseIsRoot => write!(f, "base is root"),
            CanonicalizationError => write!(f, "canonicalization error"),
        }
    }
}

/// Errors returned when creating a `NodeId` out of a string
#[derive(Debug, Clone)]
pub enum NodeIdError {
    NodeIdRequired,
}

impl From<NodeIdError> for ValueErrorKind {
    fn from(e: NodeIdError) -> ValueErrorKind {
        match e {
            NodeIdError::NodeIdRequired => {
                ValueErrorKind::value_error("fragment identifier required")
            }
        }
    }
}

/// Errors that can happen while loading an SVG document.
///
/// All of these codes are for unrecoverable errors that keep an SVG document from being
/// fully loaded and parsed.  Note that SVG is very lenient with respect to document
/// structure and the syntax of CSS property values; most errors there will not lead to a
/// `LoadingError`.  To see those errors, you may want to set the `RSVG_LOG=1` environment
/// variable.
///
/// I/O errors get reported in the `Glib` variant, since librsvg uses GIO internally for
/// all input/output.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub enum LoadingError {
    /// XML syntax error.
    XmlParseError(String),

    /// Not enough memory to load the document.
    OutOfMemory(String),

    /// A malformed or disallowed URL was used.
    BadUrl,

    /// An invalid stylesheet was used.
    BadCss,

    /// There is no `<svg>` root element in the XML.
    NoSvgRoot,

    /// I/O error.
    Io(String),

    /// A particular implementation-defined limit was exceeded.
    LimitExceeded(ImplementationLimit),

    /// Catch-all for loading errors.
    Other(String),
}

/// Errors for implementation-defined limits, to mitigate malicious SVG documents.
///
/// These get emitted as `LoadingError::LimitExceeded` or `RenderingError::LimitExceeded`.
/// The limits are present to mitigate malicious SVG documents which may try to exhaust
/// all available memory, or which would use large amounts of CPU time.
#[non_exhaustive]
#[derive(Debug, Copy, Clone)]
pub enum ImplementationLimit {
    /// Document exceeded the maximum number of times that elements
    /// can be referenced through URL fragments.
    ///
    /// This is a mitigation for malicious documents that attempt to
    /// consume exponential amounts of CPU time by creating millions
    /// of references to SVG elements.  For example, the `<use>` and
    /// `<pattern>` elements allow referencing other elements, which
    /// can in turn reference other elements.  This can be used to
    /// create documents which would require exponential amounts of
    /// CPU time to be rendered.
    ///
    /// Librsvg deals with both cases by placing a limit on how many
    /// references will be resolved during the SVG rendering process,
    /// that is, how many `url(#foo)` will be resolved.
    ///
    /// These malicious documents are similar to the XML
    /// [billion laughs attack], but done with SVG's referencing features.
    ///
    /// See issues
    /// [#323](https://gitlab.gnome.org/GNOME/librsvg/issues/323) and
    /// [#515](https://gitlab.gnome.org/GNOME/librsvg/issues/515) for
    /// examples for the `<use>` and `<pattern>` elements,
    /// respectively.
    ///
    /// [billion laughs attack]: https://bitbucket.org/tiran/defusedxml
    TooManyReferencedElements,

    /// Document exceeded the maximum number of elements that can be loaded.
    ///
    /// This is a mitigation for SVG files which create millions of
    /// elements in an attempt to exhaust memory.  Librsvg does not't
    /// allow loading more than a certain number of elements during
    /// the initial loading process.
    TooManyLoadedElements,
}

impl error::Error for LoadingError {}

impl fmt::Display for LoadingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            LoadingError::XmlParseError(ref s) => write!(f, "XML parse error: {}", s),
            LoadingError::OutOfMemory(ref s) => write!(f, "out of memory: {}", s),
            LoadingError::BadUrl => write!(f, "invalid URL"),
            LoadingError::BadCss => write!(f, "invalid CSS"),
            LoadingError::NoSvgRoot => write!(f, "XML does not have <svg> root"),
            LoadingError::Io(ref s) => write!(f, "I/O error: {}", s),
            LoadingError::LimitExceeded(ref l) => write!(f, "{}", l),
            LoadingError::Other(ref s) => write!(f, "{}", s),
        }
    }
}

impl From<glib::Error> for LoadingError {
    fn from(e: glib::Error) -> LoadingError {
        // FIXME: this is somewhat fishy; not all GError are I/O errors, but in librsvg
        // most GError do come from gio.  Some come from GdkPixbufLoader, though.
        LoadingError::Io(format!("{}", e))
    }
}

impl From<IoError> for LoadingError {
    fn from(e: IoError) -> LoadingError {
        match e {
            IoError::BadDataUrl => LoadingError::BadUrl,
            IoError::Glib(e) => LoadingError::Io(format!("{}", e)),
        }
    }
}

impl fmt::Display for ImplementationLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            ImplementationLimit::TooManyReferencedElements => write!(
                f,
                "exceeded more than {} referenced elements",
                limits::MAX_REFERENCED_ELEMENTS
            ),

            ImplementationLimit::TooManyLoadedElements => write!(
                f,
                "cannot load more than {} XML elements",
                limits::MAX_LOADED_ELEMENTS
            ),
        }
    }
}
