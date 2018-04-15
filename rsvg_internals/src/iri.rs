use cssparser::{Parser, ParserInput};
use std::str;

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

impl Parse for IRI {
    type Data = ();
    type Err = ParseError;

    fn parse(s: &str, _: Self::Data) -> Result<IRI, ParseError> {
        match s.trim() {
            "none" => Ok(IRI::None),

            _ => {
                let mut input = ParserInput::new(s);
                let mut parser = Parser::new(&mut input);

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_none() {
        assert_eq!(IRI::parse("none", ()), Ok(IRI::None));
    }

    #[test]
    fn parses_url() {
        assert_eq!(
            IRI::parse("url(foo)", ()),
            Ok(IRI::Resource("foo".to_string()))
        );

        // be permissive if the closing ) is missing
        assert_eq!(IRI::parse("url(", ()), Ok(IRI::Resource("".to_string())));
        assert_eq!(
            IRI::parse("url(foo", ()),
            Ok(IRI::Resource("foo".to_string()))
        );

        assert!(IRI::parse("", ()).is_err());
        assert!(IRI::parse("foo", ()).is_err());
        assert!(IRI::parse("url(foo)bar", ()).is_err());
    }
}
