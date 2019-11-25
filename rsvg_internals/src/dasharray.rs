use cssparser::Parser;

use crate::error::*;
use crate::length::*;
use crate::parsers::{CssParserExt, Parse};

#[derive(Debug, PartialEq, Clone)]
pub enum Dasharray {
    None,
    Array(Vec<Length<Both>>),
}

impl Default for Dasharray {
    fn default() -> Dasharray {
        Dasharray::None
    }
}

impl Parse for Dasharray {
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<Dasharray, ValueErrorKind> {
        if parser
            .try_parse(|p| p.expect_ident_matching("none"))
            .is_ok()
        {
            return Ok(Dasharray::None);
        }

        let mut dasharray = Vec::new();

        loop {
            let d = Length::<Both>::parse(parser).and_then(Length::<Both>::check_nonnegative)?;
            dasharray.push(d);

            if parser.is_exhausted() {
                break;
            }

            parser.optional_comma();
        }

        Ok(Dasharray::Array(dasharray))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_dash_array() {
        // helper to cut down boilderplate
        let length_parse = |s| Length::<Both>::parse_str(s).unwrap();

        let expected = Dasharray::Array(vec![
            length_parse("1"),
            length_parse("2in"),
            length_parse("3"),
            length_parse("4%"),
        ]);

        let sample_1 = Dasharray::Array(vec![length_parse("10"), length_parse("6")]);

        let sample_2 = Dasharray::Array(vec![
            length_parse("5"),
            length_parse("5"),
            length_parse("20"),
        ]);

        let sample_3 = Dasharray::Array(vec![
            length_parse("10px"),
            length_parse("20px"),
            length_parse("20px"),
        ]);

        let sample_4 = Dasharray::Array(vec![
            length_parse("25"),
            length_parse("5"),
            length_parse("5"),
            length_parse("5"),
        ]);

        let sample_5 = Dasharray::Array(vec![length_parse("3.1415926"), length_parse("8")]);
        let sample_6 = Dasharray::Array(vec![length_parse("5"), length_parse("3.14")]);
        let sample_7 = Dasharray::Array(vec![length_parse("2")]);

        assert_eq!(Dasharray::parse_str("none").unwrap(), Dasharray::None);
        assert_eq!(Dasharray::parse_str("1 2in,3 4%").unwrap(), expected);
        assert_eq!(Dasharray::parse_str("10,6").unwrap(), sample_1);
        assert_eq!(Dasharray::parse_str("5,5,20").unwrap(), sample_2);
        assert_eq!(Dasharray::parse_str("10px 20px 20px").unwrap(), sample_3);
        assert_eq!(Dasharray::parse_str("25  5 , 5 5").unwrap(), sample_4);
        assert_eq!(Dasharray::parse_str("3.1415926,8").unwrap(), sample_5);
        assert_eq!(Dasharray::parse_str("5, 3.14").unwrap(), sample_6);
        assert_eq!(Dasharray::parse_str("2").unwrap(), sample_7);

        // Negative numbers
        assert_eq!(
            Dasharray::parse_str("20,40,-20"),
            Err(ValueErrorKind::Value(String::from(
                "value must be non-negative"
            )))
        );

        // Empty dash_array
        assert!(Dasharray::parse_str("").is_err());
        assert!(Dasharray::parse_str("\t  \n     ").is_err());
        assert!(Dasharray::parse_str(",,,").is_err());
        assert!(Dasharray::parse_str("10,  \t, 20 \n").is_err());

        // No trailing commas allowed, parse error
        assert!(Dasharray::parse_str("10,").is_err());

        // A comma should be followed by a number
        assert!(Dasharray::parse_str("20,,10").is_err());
    }
}
