//! Parser for an Accept-Language HTTP header.

use language_tags::{LanguageTag, ParseError};

use std::str::FromStr;

#[derive(Debug, PartialEq)]
struct Weight(Option<f32>);

impl Weight {
    fn numeric(&self) -> f32 {
        self.0.unwrap_or(1.0)
    }
}

#[derive(Debug, PartialEq)]
struct Item {
    tag: LanguageTag,
    weight: Weight,
}

/// Stores a parsed version of an HTTP Accept-Language header.
///
/// https://datatracker.ietf.org/doc/html/rfc7231#section-5.3.5
#[derive(Debug, PartialEq)]
pub struct AcceptLanguage(Box<[Item]>);

/// Errors when parsing an `AcceptLanguage`.
#[derive(Debug, PartialEq)]
pub enum Error {
    NoElements,
    InvalidCharacters,
    InvalidLanguageTag(ParseError),
    InvalidWeight,
}

/// Optional whitespace, Space or Tab, per https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.3
const OWS: [char; 2] = ['\x20', '\x09'];

impl AcceptLanguage {
    pub fn parse(s: &str) -> Result<AcceptLanguage, Error> {
        if !s.is_ascii() {
            return Err(Error::InvalidCharacters);
        }

        let mut items = Vec::new();

        for val in s.split(',') {
            let trimmed = val.trim_matches(&OWS[..]);
            if trimmed.len() == 0 {
                continue;
            }

            items.push(Item::parse(trimmed)?);
        }

        if items.len() == 0 {
            Err(Error::NoElements)
        } else {
            Ok(AcceptLanguage(items.into_boxed_slice()))
        }
    }
}

impl Item {
    fn parse(s: &str) -> Result<Item, Error> {
        let semicolon_pos = s.find(';');

        let (before_semicolon, after_semicolon) = if let Some(semi) = semicolon_pos {
            (&s[..semi], Some(&s[semi + 1..]))
        } else {
            (s, None)
        };

        let tag = LanguageTag::parse(before_semicolon).map_err(Error::InvalidLanguageTag)?;

        let weight;

        if let Some(quality) = after_semicolon {
            let quality = quality.trim_start_matches(&OWS[..]);

            let number = if let Some(qvalue) = quality.strip_prefix("q=") {
                if qvalue.starts_with(&['0', '1'][..]) {
                    let first_digit = qvalue.chars().next().unwrap();

                    if let Some(decimals) = qvalue[1..].strip_prefix(".") {
                        if first_digit == '0'
                            && decimals.len() <= 3
                            && decimals.chars().all(|c| c.is_digit(10))
                        {
                            qvalue
                        } else if first_digit == '1'
                            && decimals.len() <= 3
                            && decimals.chars().all(|c| c == '0')
                        {
                            qvalue
                        } else {
                            return Err(Error::InvalidWeight);
                        }
                    } else {
                        qvalue
                    }
                } else {
                    return Err(Error::InvalidWeight);
                }
            } else {
                return Err(Error::InvalidWeight);
            };

            weight = Weight(Some(
                f32::from_str(number).map_err(|_| Error::InvalidWeight)?,
            ));
        } else {
            weight = Weight(None);
        }

        Ok(Item { tag, weight })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_accept_language() {
        // plain tag
        assert_eq!(
            AcceptLanguage::parse("es-MX").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(None)
                }]
                .into_boxed_slice()
            )
        );

        // with quality
        assert_eq!(
            AcceptLanguage::parse("es-MX;q=1").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(Some(1.0))
                }]
                .into_boxed_slice()
            )
        );

        // with quality
        assert_eq!(
            AcceptLanguage::parse("es-MX;q=0").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(Some(0.0))
                }]
                .into_boxed_slice()
            )
        );

        // zero decimals are allowed
        assert_eq!(
            AcceptLanguage::parse("es-MX;q=0.").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(Some(0.0))
                }]
                .into_boxed_slice()
            )
        );

        // zero decimals are allowed
        assert_eq!(
            AcceptLanguage::parse("es-MX;q=1.").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(Some(1.0))
                }]
                .into_boxed_slice()
            )
        );

        // one decimal
        assert_eq!(
            AcceptLanguage::parse("es-MX;q=1.0").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(Some(1.0))
                }]
                .into_boxed_slice()
            )
        );

        // two decimals
        assert_eq!(
            AcceptLanguage::parse("es-MX;q=1.00").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(Some(1.0))
                }]
                .into_boxed_slice()
            )
        );

        // three decimals
        assert_eq!(
            AcceptLanguage::parse("es-MX;q=1.000").unwrap(),
            AcceptLanguage(
                vec![Item {
                    tag: LanguageTag::parse("es-MX").unwrap(),
                    weight: Weight(Some(1.0))
                }]
                .into_boxed_slice()
            )
        );

        // multiple elements
        assert_eq!(
            AcceptLanguage::parse("es-MX, en; q=0.5").unwrap(),
            AcceptLanguage(
                vec![
                    Item {
                        tag: LanguageTag::parse("es-MX").unwrap(),
                        weight: Weight(None)
                    },
                    Item {
                        tag: LanguageTag::parse("en").unwrap(),
                        weight: Weight(Some(0.5))
                    },
                ]
                .into_boxed_slice()
            )
        );

        // superfluous whitespace
        assert_eq!(
            AcceptLanguage::parse(",es-MX;q=1.000  , en; q=0.125  ,  ,").unwrap(),
            AcceptLanguage(
                vec![
                    Item {
                        tag: LanguageTag::parse("es-MX").unwrap(),
                        weight: Weight(Some(1.0))
                    },
                    Item {
                        tag: LanguageTag::parse("en").unwrap(),
                        weight: Weight(Some(0.125))
                    },
                ]
                .into_boxed_slice()
            )
        );
    }

    #[test]
    fn empty_lists() {
        assert!(matches!(AcceptLanguage::parse(""), Err(Error::NoElements)));

        assert!(matches!(AcceptLanguage::parse(","), Err(Error::NoElements)));

        assert!(matches!(
            AcceptLanguage::parse(", , ,,,"),
            Err(Error::NoElements)
        ));
    }

    #[test]
    fn ascii_only() {
        assert!(matches!(
            AcceptLanguage::parse("ës"),
            Err(Error::InvalidCharacters)
        ));
    }

    #[test]
    fn invalid_tag() {
        assert!(matches!(
            AcceptLanguage::parse("no_underscores"),
            Err(Error::InvalidLanguageTag(_))
        ));
    }

    #[test]
    fn invalid_weight() {
        assert!(matches!(
            AcceptLanguage::parse("es;"),
            Err(Error::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q"),
            Err(Error::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q="),
            Err(Error::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=2"),
            Err(Error::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=1.1"),
            Err(Error::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=1.12"),
            Err(Error::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=1.123"),
            Err(Error::InvalidWeight)
        ));

        // Up to three decimals allowed per RFC 7231
        assert!(matches!(
            AcceptLanguage::parse("es;q=0.1234"),
            Err(Error::InvalidWeight)
        ));
    }
}
