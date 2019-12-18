use cssparser::{Parser, Token};
use markup5ever::QualName;

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
    FilterOutput(String),
}

/// https://www.w3.org/TR/css-values-4/#custom-idents
#[derive(Debug, PartialEq)]
pub struct CustomIdent(String);

impl Input {
    pub fn parse(attr: QualName, s: &str) -> Result<Input, NodeError> {
        match s {
            "SourceGraphic" => Ok(Input::SourceGraphic),
            "SourceAlpha" => Ok(Input::SourceAlpha),
            "BackgroundImage" => Ok(Input::BackgroundImage),
            "BackgroundAlpha" => Ok(Input::BackgroundAlpha),
            "FillPaint" => Ok(Input::FillPaint),
            "StrokePaint" => Ok(Input::StrokePaint),
            s if !s.is_empty() => Ok(Input::FilterOutput(s.to_string())),
            _ => Err(ValueErrorKind::parse_error("invalid value")).attribute(attr),
        }
    }
}

impl Parse for CustomIdent {
    fn parse(parser: &mut Parser<'_, '_>) -> Result<Self, ValueErrorKind> {
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
