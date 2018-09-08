use cssparser::Parser;

use parsers::Parse;
use parsers::ParseError;

#[derive(Debug, Clone, PartialEq)]
pub enum IRI {
    None,
    Resource(String),
}

impl Default for IRI {
    fn default() -> IRI {
        IRI::None
    }
}

impl IRI {
    /// Returns the contents of an `IRI::Resource`, or `None`
    pub fn get(&self) -> Option<&str> {
        match *self {
            IRI::None => None,
            IRI::Resource(ref s) => Some(s.as_ref()),
        }
    }
}

impl Parse for IRI {
    type Data = ();
    type Err = ParseError;

    fn parse(parser: &mut Parser<'_, '_>, _: Self::Data) -> Result<IRI, ParseError> {
        if parser.try(|i| i.expect_ident_matching("none")).is_ok() {
            Ok(IRI::None)
        } else {
            let url = parser
                .expect_url()
                .map_err(|_| ParseError::new("expected url"))?;

            parser
                .expect_exhausted()
                .map_err(|_| ParseError::new("expected url"))?;

            Ok(IRI::Resource(url.as_ref().to_owned()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_none() {
        assert_eq!(IRI::parse_str("none", ()), Ok(IRI::None));
    }

    #[test]
    fn parses_url() {
        assert_eq!(
            IRI::parse_str("url(foo)", ()),
            Ok(IRI::Resource("foo".to_string()))
        );

        // be permissive if the closing ) is missing
        assert_eq!(
            IRI::parse_str("url(", ()),
            Ok(IRI::Resource("".to_string()))
        );
        assert_eq!(
            IRI::parse_str("url(foo", ()),
            Ok(IRI::Resource("foo".to_string()))
        );

        assert!(IRI::parse_str("", ()).is_err());
        assert!(IRI::parse_str("foo", ()).is_err());
        assert!(IRI::parse_str("url(foo)bar", ()).is_err());
    }

    #[test]
    fn get() {
        assert_eq!(IRI::None.get(), None);
        assert_eq!(IRI::Resource(String::from("foo")).get(), Some("foo"));
    }
}
