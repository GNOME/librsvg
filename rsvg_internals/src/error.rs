use std::error;
use std::fmt;

use cairo;
use cssparser::BasicParseError;
use glib;

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

#[derive(Clone)]
pub enum LoadingError {
    Cairo(cairo::Status),
    EmptyData,
    Glib(glib::Error),
    Unknown,
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
