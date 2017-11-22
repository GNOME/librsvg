use std::fmt;
use std::error;

use parsers::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum AttributeError {
    // parse error
    Parse (ParseError),

    // invalid value
    Value (String)
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeError {
    attr_name: String,
    err:       AttributeError
}

impl NodeError {
    pub fn parse_error (attr_name: &str, error: ParseError) -> NodeError {
        NodeError {
            attr_name: attr_name.to_string (),
            err: AttributeError::Parse (error)
        }
    }

    pub fn value_error (attr_name: &str, description: &str) -> NodeError {
        NodeError {
            attr_name: attr_name.to_string (),
            err: AttributeError::Value (description.to_string ())
        }
    }

    pub fn attribute_error (attr_name: &str, error: AttributeError) -> NodeError {
        NodeError {
            attr_name: attr_name.to_string (),
            err: error
        }
    }
}

impl error::Error for NodeError {
    fn description (&self) -> &str {
        match self.err {
            AttributeError::Parse (_) => &"parse error",
            AttributeError::Value (_) => &"invalid attribute value"
        }
    }
}

impl fmt::Display for NodeError {
    fn fmt (&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.err {
            AttributeError::Parse (ref n) => write! (f,
                                                     "error parsing value for attribute \"{}\": {}",
                                                     self.attr_name,
                                                     n.display),

            AttributeError::Value (ref s) => write! (f,
                                                     "invalid value for attribute \"{}\": {}",
                                                     self.attr_name,
                                                     s)
        }
    }
}

impl From<ParseError> for AttributeError {
    fn from (pe: ParseError) -> AttributeError {
        AttributeError::Parse (pe)
    }
}

#[cfg(test)]
pub fn is_parse_error<T> (r: &Result<T, AttributeError>) -> bool {
    match *r {
        Err (AttributeError::Parse (_)) => true,
        _ => false
    }
}

#[cfg(test)]
pub fn is_value_error<T> (r: &Result<T, AttributeError>) -> bool {
    match *r {
        Err (AttributeError::Value (_)) => true,
        _ => false
    }
}
