//! CSS number-list values.

use cssparser::{Parser, ParserInput};

use crate::parsers::{optional_comma, Parse};

#[derive(Eq, PartialEq)]
pub enum NumberListLength {
    Exact(usize),
    Unbounded,
}

#[derive(Debug, PartialEq)]
pub enum NumberListError {
    IncorrectNumberOfElements,
    Parse(String),
}

#[derive(Debug, PartialEq)]
pub struct NumberList(pub Vec<f64>);

impl NumberList {
    pub fn parse(
        parser: &mut Parser<'_, '_>,
        length: NumberListLength,
    ) -> Result<NumberList, NumberListError> {
        let mut v = match length {
            NumberListLength::Exact(l) if l > 0 => Vec::<f64>::with_capacity(l),
            NumberListLength::Exact(_) => unreachable!(),
            NumberListLength::Unbounded => Vec::<f64>::new(),
        };

        if parser.is_exhausted() && length == NumberListLength::Unbounded {
            return Ok(NumberList(v));
        }

        for i in 0.. {
            if i != 0 {
                optional_comma(parser);
            }

            v.push(f64::parse(parser).map_err(|_| {
                NumberListError::Parse("expected number".to_string())
            })?);

            if let NumberListLength::Exact(l) = length {
                if i + 1 == l {
                    break;
                }
            }

            if parser.is_exhausted() {
                match length {
                    NumberListLength::Exact(l) => {
                        if i + 1 == l {
                            break;
                        }
                    }
                    _ => break,
                }
            }
        }

        parser
            .expect_exhausted()
            .map_err(|_| NumberListError::IncorrectNumberOfElements)?;

        Ok(NumberList(v))
    }

    pub fn parse_str(s: &str, length: NumberListLength) -> Result<NumberList, NumberListError> {
        let mut input = ParserInput::new(s);
        let mut parser = Parser::new(&mut input);

        Self::parse(&mut parser, length).and_then(|r| {
            // FIXME: parser.expect_exhausted()?;
            Ok(r)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_number_list() {
        assert_eq!(
            NumberList::parse_str("5", NumberListLength::Exact(1)),
            Ok(NumberList(vec![5.0]))
        );

        assert_eq!(
            NumberList::parse_str("1 2 3 4", NumberListLength::Exact(4)),
            Ok(NumberList(vec![1.0, 2.0, 3.0, 4.0]))
        );

        assert_eq!(
            NumberList::parse_str("", NumberListLength::Unbounded),
            Ok(NumberList(vec![]))
        );

        assert_eq!(
            NumberList::parse_str("1, 2, 3.0, 4, 5", NumberListLength::Unbounded),
            Ok(NumberList(vec![1.0, 2.0, 3.0, 4.0, 5.0]))
        );
    }

    #[test]
    fn errors_on_invalid_number_list() {
        // empty
        assert!(NumberList::parse_str("", NumberListLength::Exact(1)).is_err());

        // garbage
        assert!(NumberList::parse_str("foo", NumberListLength::Exact(1)).is_err());
        assert!(NumberList::parse_str("1foo", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1 foo", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1 foo 2", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1,foo", NumberListLength::Exact(2)).is_err());

        // too many
        assert!(NumberList::parse_str("1 2", NumberListLength::Exact(1)).is_err());

        // extra token
        assert!(NumberList::parse_str("1,", NumberListLength::Exact(1)).is_err());
        assert!(NumberList::parse_str("1,", NumberListLength::Exact(1)).is_err());
        assert!(NumberList::parse_str("1,", NumberListLength::Unbounded).is_err());

        // too few
        assert!(NumberList::parse_str("1", NumberListLength::Exact(2)).is_err());
        assert!(NumberList::parse_str("1 2", NumberListLength::Exact(3)).is_err());
    }
}
