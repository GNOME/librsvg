//! Parser for an Accept-Language HTTP header.

use language_tags::{LanguageTag, ParseError};
use locale_config::{LanguageRange, Locale};

use std::error;
use std::fmt;
use std::str::FromStr;

/// Used to set the language for rendering.
///
/// SVG documents can use the `<switch>` element, whose children have a `systemLanguage`
/// attribute; only the first child which has a `systemLanguage` that matches the
/// preferred languages will be rendered.
///
/// This enum, used with `CairoRenderer::with_language`, configures how to obtain the
/// user's prefererred languages.
pub enum Language {
    /// Use the Unix environment variables `LANGUAGE`, `LC_ALL`, `LC_MESSAGES` and `LANG` to obtain the
    /// user's language.  This uses [`g_get_language_names()`][ggln] underneath.
    ///
    /// [ggln]: https://docs.gtk.org/glib/func.get_language_names.html
    FromEnvironment,
    AcceptLanguage(AcceptLanguage),
}

/// `Language` but with the environment's locale converted to something we can use.
#[derive(Clone)]
pub enum UserLanguage {
    LanguageTags(LanguageTags),
    AcceptLanguage(AcceptLanguage),
}

#[derive(Clone, Debug, PartialEq)]
struct Weight(Option<f32>);

impl Weight {
    fn numeric(&self) -> f32 {
        self.0.unwrap_or(1.0)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Item {
    tag: LanguageTag,
    weight: Weight,
}

/// Stores a parsed version of an HTTP Accept-Language header.
///
/// RFC 7231: <https://datatracker.ietf.org/doc/html/rfc7231#section-5.3.5>
#[derive(Clone, Debug, PartialEq)]
pub struct AcceptLanguage(Box<[Item]>);

/// Errors when parsing an `AcceptLanguage`.
#[derive(Debug, PartialEq)]
pub enum AcceptLanguageError {
    NoElements,
    InvalidCharacters,
    InvalidLanguageTag(ParseError),
    InvalidWeight,
}

impl error::Error for AcceptLanguageError {}

impl fmt::Display for AcceptLanguageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoElements => write!(f, "no language tags in list"),
            Self::InvalidCharacters => write!(f, "invalid characters in language list"),
            Self::InvalidLanguageTag(e) => write!(f, "invalid language tag: {}", e),
            Self::InvalidWeight => write!(f, "invalid q= weight"),
        }
    }
}

/// Optional whitespace, Space or Tab, per RFC 7230.
///
/// RFC 7230: <https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.3>
const OWS: [char; 2] = ['\x20', '\x09'];

impl AcceptLanguage {
    pub fn parse(s: &str) -> Result<AcceptLanguage, AcceptLanguageError> {
        if !s.is_ascii() {
            return Err(AcceptLanguageError::InvalidCharacters);
        }

        let mut items = Vec::new();

        for val in s.split(',') {
            let trimmed = val.trim_matches(&OWS[..]);
            if trimmed.is_empty() {
                continue;
            }

            items.push(Item::parse(trimmed)?);
        }

        if items.is_empty() {
            Err(AcceptLanguageError::NoElements)
        } else {
            Ok(AcceptLanguage(items.into_boxed_slice()))
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (&LanguageTag, f32)> {
        self.0.iter().map(|item| (&item.tag, item.weight.numeric()))
    }

    fn any_matches(&self, tag: &LanguageTag) -> bool {
        self.iter().any(|(self_tag, _weight)| tag.matches(self_tag))
    }
}

impl Item {
    fn parse(s: &str) -> Result<Item, AcceptLanguageError> {
        let semicolon_pos = s.find(';');

        let (before_semicolon, after_semicolon) = if let Some(semi) = semicolon_pos {
            (&s[..semi], Some(&s[semi + 1..]))
        } else {
            (s, None)
        };

        let tag = LanguageTag::parse(before_semicolon)
            .map_err(AcceptLanguageError::InvalidLanguageTag)?;

        let weight = if let Some(quality) = after_semicolon {
            let quality = quality.trim_start_matches(&OWS[..]);

            let number = if let Some(qvalue) = quality.strip_prefix("q=") {
                if qvalue.starts_with(&['0', '1'][..]) {
                    let first_digit = qvalue.chars().next().unwrap();

                    if let Some(decimals) = qvalue[1..].strip_prefix('.') {
                        if (first_digit == '0'
                            && decimals.len() <= 3
                            && decimals.chars().all(|c| c.is_digit(10)))
                            || (first_digit == '1'
                                && decimals.len() <= 3
                                && decimals.chars().all(|c| c == '0'))
                        {
                            qvalue
                        } else {
                            return Err(AcceptLanguageError::InvalidWeight);
                        }
                    } else {
                        qvalue
                    }
                } else {
                    return Err(AcceptLanguageError::InvalidWeight);
                }
            } else {
                return Err(AcceptLanguageError::InvalidWeight);
            };

            Weight(Some(
                f32::from_str(number).map_err(|_| AcceptLanguageError::InvalidWeight)?,
            ))
        } else {
            Weight(None)
        };

        Ok(Item { tag, weight })
    }
}

/// A list of BCP47 language tags.
///
/// RFC 5664: <https://www.rfc-editor.org/info/rfc5664>
#[derive(Debug, Clone, PartialEq)]
pub struct LanguageTags(Vec<LanguageTag>);

impl LanguageTags {
    pub fn empty() -> Self {
        LanguageTags(Vec::new())
    }

    /// Converts a `Locale` to a set of language tags.
    pub fn from_locale(locale: &Locale) -> Result<LanguageTags, String> {
        let mut tags = Vec::new();

        for locale_range in locale.tags_for("messages") {
            if locale_range == LanguageRange::invariant() {
                continue;
            }

            let str_locale_range = locale_range.as_ref();

            let locale_tag = LanguageTag::from_str(str_locale_range).map_err(|e| {
                format!(
                    "invalid language tag \"{}\" in locale: {}",
                    str_locale_range, e
                )
            })?;

            if !locale_tag.is_language_range() {
                return Err(format!(
                    "language tag \"{}\" is not a language range",
                    locale_tag
                ));
            }

            tags.push(locale_tag);
        }

        Ok(LanguageTags(tags))
    }

    pub fn from(tags: Vec<LanguageTag>) -> LanguageTags {
        LanguageTags(tags)
    }

    pub fn iter(&self) -> impl Iterator<Item = &LanguageTag> {
        self.0.iter()
    }

    pub fn any_matches(&self, language_tag: &LanguageTag) -> bool {
        self.0.iter().any(|tag| tag.matches(language_tag))
    }
}

impl UserLanguage {
    pub fn new(language: &Language) -> UserLanguage {
        match *language {
            Language::FromEnvironment => UserLanguage::LanguageTags(
                LanguageTags::from_locale(&locale_from_environment())
                    .map_err(|s| {
                        rsvg_log!("could not convert locale to language tags: {}", s);
                    })
                    .unwrap_or_else(|_| LanguageTags::empty()),
            ),

            Language::AcceptLanguage(ref a) => UserLanguage::AcceptLanguage(a.clone()),
        }
    }

    pub fn any_matches(&self, tags: &LanguageTags) -> bool {
        match *self {
            UserLanguage::LanguageTags(ref language_tags) => {
                tags.iter().any(|tag| language_tags.any_matches(tag))
            }
            UserLanguage::AcceptLanguage(ref accept_language) => {
                tags.iter().any(|tag| accept_language.any_matches(tag))
            }
        }
    }
}

/// Gets the user's preferred locale from the environment and
/// translates it to a `Locale` with `LanguageRange` fallbacks.
///
/// The `Locale::current()` call only contemplates a single language,
/// but glib is smarter, and `g_get_langauge_names()` can provide
/// fallbacks, for example, when LC_MESSAGES="en_US.UTF-8:de" (USA
/// English and German).  This function converts the output of
/// `g_get_language_names()` into a `Locale` with appropriate
/// fallbacks.
fn locale_from_environment() -> Locale {
    let mut locale = Locale::invariant();

    for name in glib::language_names() {
        let name = name.as_str();
        if let Ok(range) = LanguageRange::from_unix(name) {
            locale.add(&range);
        }
    }

    locale
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
        assert!(matches!(
            AcceptLanguage::parse(""),
            Err(AcceptLanguageError::NoElements)
        ));

        assert!(matches!(
            AcceptLanguage::parse(","),
            Err(AcceptLanguageError::NoElements)
        ));

        assert!(matches!(
            AcceptLanguage::parse(", , ,,,"),
            Err(AcceptLanguageError::NoElements)
        ));
    }

    #[test]
    fn ascii_only() {
        assert!(matches!(
            AcceptLanguage::parse("ës"),
            Err(AcceptLanguageError::InvalidCharacters)
        ));
    }

    #[test]
    fn invalid_tag() {
        assert!(matches!(
            AcceptLanguage::parse("no_underscores"),
            Err(AcceptLanguageError::InvalidLanguageTag(_))
        ));
    }

    #[test]
    fn invalid_weight() {
        assert!(matches!(
            AcceptLanguage::parse("es;"),
            Err(AcceptLanguageError::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q"),
            Err(AcceptLanguageError::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q="),
            Err(AcceptLanguageError::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=2"),
            Err(AcceptLanguageError::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=1.1"),
            Err(AcceptLanguageError::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=1.12"),
            Err(AcceptLanguageError::InvalidWeight)
        ));
        assert!(matches!(
            AcceptLanguage::parse("es;q=1.123"),
            Err(AcceptLanguageError::InvalidWeight)
        ));

        // Up to three decimals allowed per RFC 7231
        assert!(matches!(
            AcceptLanguage::parse("es;q=0.1234"),
            Err(AcceptLanguageError::InvalidWeight)
        ));
    }

    #[test]
    fn iter() {
        let accept_language = AcceptLanguage::parse("es-MX, en; q=0.5").unwrap();
        let mut iter = accept_language.iter();

        let (tag, weight) = iter.next().unwrap();
        assert_eq!(*tag, LanguageTag::parse("es-MX").unwrap());
        assert_eq!(weight, 1.0);

        let (tag, weight) = iter.next().unwrap();
        assert_eq!(*tag, LanguageTag::parse("en").unwrap());
        assert_eq!(weight, 0.5);

        assert!(iter.next().is_none());
    }
}
