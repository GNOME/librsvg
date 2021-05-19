//! Parser for an Accept-Language HTTP header.

use language_tags::LanguageTag;

struct Weight(Option<f32>);

impl Weight {
    fn numeric(&self) -> f32 {
        self.0.unwrap_or(1.0)
    }
}

struct Item {
    tag: LanguageTag,
    weight: Weight,
}

/// Stores a parsed version of an HTTP Accept-Language header.
///
/// https://datatracker.ietf.org/doc/html/rfc7231#section-5.3.5
pub struct AcceptLanguage(Box<[Item]>);

/// Errors when parsing an `AcceptLanguage`.
#[derive(Debug, PartialEq)]
pub enum Error {
    NoElements,
}

impl AcceptLanguage {
    pub fn parse(s: &str) -> Result<AcceptLanguage, Error> {
        let mut items = Vec::new();

        for val in s.split(',') {
            
        }

        if items.len() == 0 {
            Err(Error::NoElements)
        } else {
            Ok(AcceptLanguage(items.into_boxed_slice()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_lists_yield_error() {
        assert!(matches!(
            AcceptLanguage::parse(""),
            Err(Error::NoElements)
        ));

        assert!(matches!(
            AcceptLanguage::parse(","),
            Err(Error::NoElements)
        ));

        assert!(matches!(
            AcceptLanguage::parse(", , ,,,"),
            Err(Error::NoElements)
        ));
    }
}
