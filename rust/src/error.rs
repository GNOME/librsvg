use std::fmt;
use std::error;

use parsers::ParseError;

#[derive(Debug)]
pub enum AttributeError {
    // parse error
    Parse (ParseError),

    // invalid value
    Value (String)
}

#[derive(Debug)]
pub struct NodeError {
    attr_name: &'static str,
    err:       AttributeError
}

impl NodeError {
    pub fn parse_error (attr_name: &'static str, error: ParseError) -> NodeError {
        NodeError {
            attr_name: attr_name,
            err: AttributeError::Parse (error)
        }
    }

    pub fn value_error (attr_name: &'static str, description: String) -> NodeError {
        NodeError {
            attr_name: attr_name,
            err: AttributeError::Value (description)
        }
    }
}

impl error::Error for NodeError {
    fn description (&self) -> &str {
        match self.err {
            AttributeError::Parse (ref n) => &"parse error",
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

