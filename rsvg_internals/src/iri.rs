use cssparser::Parser;

use allowed_url::{Fragment, Href};
use parsers::Parse;
use parsers::ParseError;

/// Used where style properties take a funciri or "none"
///
/// This is not to be used for values which don't come from properties.
/// For example, the `xlink:href` attribute in the `<image>` element
/// does not take a funciri value (which looks like `url(...)`), but rather
/// it takes a plain URL.  Use the `Href` type in that case.
#[derive(Debug, Clone, PartialEq)]
pub enum IRI {
    None,
    Resource(Fragment),
}

impl Default for IRI {
    fn default() -> IRI {
        IRI::None
    }
}

impl IRI {
    /// Returns the contents of an `IRI::Resource`, or `None`
    pub fn get(&self) -> Option<&Fragment> {
        match *self {
            IRI::None => None,
            IRI::Resource(ref f) => Some(f),
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

            let href =
                Href::with_fragment(&url).map_err(|_| ParseError::new("could not parse href"))?;

            if let Href::WithFragment(fragment) = href {
                Ok(IRI::Resource(fragment))
            } else {
                Err(ParseError::new("href requires a fragment identifier"))
            }
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
            IRI::parse_str("url(#bar)", ()),
            Ok(IRI::Resource(Fragment::new(None, "bar".to_string())))
        );

        assert_eq!(
            IRI::parse_str("url(foo#bar)", ()),
            Ok(IRI::Resource(Fragment::new(
                Some("foo".to_string()),
                "bar".to_string()
            )))
        );

        // be permissive if the closing ) is missing
        assert_eq!(
            IRI::parse_str("url(#bar", ()),
            Ok(IRI::Resource(Fragment::new(None, "bar".to_string())))
        );
        assert_eq!(
            IRI::parse_str("url(foo#bar", ()),
            Ok(IRI::Resource(Fragment::new(
                Some("foo".to_string()),
                "bar".to_string()
            )))
        );

        assert!(IRI::parse_str("", ()).is_err());
        assert!(IRI::parse_str("foo", ()).is_err());
        assert!(IRI::parse_str("url(foo)bar", ()).is_err());
    }
}
