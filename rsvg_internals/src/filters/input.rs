use cssparser::{BasicParseError, Parser, Token};

use crate::error::*;
use crate::parsers::Parse;

/// An enumeration of possible inputs for a filter primitive.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Input {
    SourceGraphic,
    SourceAlpha,
    BackgroundImage,
    BackgroundAlpha,
    FillPaint,
    StrokePaint,
    FilterOutput(CustomIdent),
}

/// https://www.w3.org/TR/css-values-4/#custom-idents
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CustomIdent(String);

impl Parse for Input {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, CssParseError<'i>> {
        parser
            .try_parse(|p| {
                Ok(parse_identifiers!(
                    p,
                    "SourceGraphic" => Input::SourceGraphic,
                    "SourceAlpha" => Input::SourceAlpha,
                    "BackgroundImage" => Input::BackgroundImage,
                    "BackgroundAlpha" => Input::BackgroundAlpha,
                    "FillPaint" => Input::FillPaint,
                    "StrokePaint" => Input::StrokePaint,
                )?)
            })
            .or_else(|_: BasicParseError| {
                let ident = CustomIdent::parse(parser)?;
                Ok(Input::FilterOutput(ident))
            })
    }
}

impl Parse for CustomIdent {
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<Self, CssParseError<'i>> {
        let loc = parser.current_source_location();
        let token = parser.next()?;

        match token {
            // CSS-wide keywords and "default" are errors here
            // https://www.w3.org/TR/css-values-4/#css-wide-keywords
            Token::Ident(ref cow) => {
                for s in &["initial", "inherit", "unset", "default"] {
                    if cow.eq_ignore_ascii_case(s) {
                        Err(loc.new_basic_unexpected_token_error(token.clone()))?
                    }
                }

                Ok(CustomIdent(cow.as_ref().to_string()))
            }

            _ => Err(loc.new_basic_unexpected_token_error(token.clone()))?,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_custom_ident() {
        assert_eq!(
            CustomIdent::parse_str("hello"),
            Ok(CustomIdent("hello".to_string()))
        );
    }

    #[test]
    fn invalid_custom_ident_yields_error() {
        assert!(CustomIdent::parse_str("initial").is_err());
        assert!(CustomIdent::parse_str("inherit").is_err());
        assert!(CustomIdent::parse_str("unset").is_err());
        assert!(CustomIdent::parse_str("default").is_err());
        assert!(CustomIdent::parse_str("").is_err());
    }
}
