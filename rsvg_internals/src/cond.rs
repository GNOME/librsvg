#[allow(unused_imports, deprecated)]
use std::ascii::AsciiExt;

use std::str::FromStr;

use glib;
use itertools::{FoldWhile, Itertools};
use language_tags::LanguageTag;
use locale_config::{LanguageRange, Locale};

use error::*;
use parsers::ParseError;

// No extensions at the moment.
static IMPLEMENTED_EXTENSIONS: &[&str] = &[];

#[derive(Debug, PartialEq)]
pub struct RequiredExtensions(pub bool);

impl RequiredExtensions {
    // Parse a requiredExtensions attribute
    // http://www.w3.org/TR/SVG/struct.html#RequiredExtensionsAttribute
    pub fn from_attribute(s: &str) -> Result<RequiredExtensions, ValueErrorKind> {
        Ok(RequiredExtensions(
            s.split_whitespace()
                .all(|f| IMPLEMENTED_EXTENSIONS.binary_search(&f).is_ok()),
        ))
    }
}

// Keep these sorted alphabetically for binary_search.
static IMPLEMENTED_FEATURES: &[&str] = &[
    "http://www.w3.org/TR/SVG11/feature#BasicFilter",
    "http://www.w3.org/TR/SVG11/feature#BasicGraphicsAttribute",
    "http://www.w3.org/TR/SVG11/feature#BasicPaintAttribute",
    "http://www.w3.org/TR/SVG11/feature#BasicStructure",
    "http://www.w3.org/TR/SVG11/feature#BasicText",
    "http://www.w3.org/TR/SVG11/feature#ConditionalProcessing",
    "http://www.w3.org/TR/SVG11/feature#ContainerAttribute",
    "http://www.w3.org/TR/SVG11/feature#Filter",
    "http://www.w3.org/TR/SVG11/feature#Gradient",
    "http://www.w3.org/TR/SVG11/feature#Image",
    "http://www.w3.org/TR/SVG11/feature#Marker",
    "http://www.w3.org/TR/SVG11/feature#Mask",
    "http://www.w3.org/TR/SVG11/feature#OpacityAttribute",
    "http://www.w3.org/TR/SVG11/feature#Pattern",
    "http://www.w3.org/TR/SVG11/feature#SVG",
    "http://www.w3.org/TR/SVG11/feature#SVG-static",
    "http://www.w3.org/TR/SVG11/feature#Shape",
    "http://www.w3.org/TR/SVG11/feature#Structure",
    "http://www.w3.org/TR/SVG11/feature#Style",
    "http://www.w3.org/TR/SVG11/feature#View",
    "org.w3c.svg.static", // deprecated SVG 1.0 feature string
];

#[derive(Debug, PartialEq)]
pub struct RequiredFeatures(pub bool);

impl RequiredFeatures {
    // Parse a requiredFeatures attribute
    // http://www.w3.org/TR/SVG/struct.html#RequiredFeaturesAttribute
    pub fn from_attribute(s: &str) -> Result<RequiredFeatures, ValueErrorKind> {
        Ok(RequiredFeatures(
            s.split_whitespace()
                .all(|f| IMPLEMENTED_FEATURES.binary_search(&f).is_ok()),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct SystemLanguage(pub bool);

impl SystemLanguage {
    /// Parse a `systemLanguage` attribute and match it against a given `Locale`
    ///
    /// The [`systemLanguage`] conditional attribute is a
    /// comma-separated list of [BCP47] Language Tags.  This function
    /// parses the attribute and matches the result against a given
    /// `locale`.  If there is a match, i.e. if the given locale
    /// supports one of the languages listed in the `systemLanguage`
    /// attribute, then the `SystemLanguage.0` will be `true`;
    /// otherwise it will be `false`.
    ///
    /// Normally, calling code will pass `&Locale::current()` for the
    /// `locale` attribute; this is the user's current locale.
    ///
    /// [`systemLanguage`]: https://www.w3.org/TR/SVG/struct.html#ConditionalProcessingSystemLanguageAttribute
    /// [BCP47]: http://www.ietf.org/rfc/bcp/bcp47.txt
    pub fn from_attribute(s: &str, locale: &Locale) -> Result<SystemLanguage, ValueErrorKind> {
        s.split(',')
            .map(LanguageTag::from_str)
            .fold_while(
                // start with no match
                Ok(SystemLanguage(false)),
                // The accumulator is Result<SystemLanguage, ValueErrorKind>
                |acc, tag_result| match tag_result {
                    Ok(language_tag) => {
                        let have_match = acc.unwrap().0;
                        if have_match {
                            FoldWhile::Continue(Ok(SystemLanguage(have_match)))
                        } else {
                            locale_accepts_language_tag(locale, &language_tag)
                                .map(|matches| FoldWhile::Continue(Ok(SystemLanguage(matches))))
                                .unwrap_or_else(|e| FoldWhile::Done(Err(e)))
                        }
                    }

                    Err(e) => FoldWhile::Done(Err(ValueErrorKind::Parse(ParseError::new(
                        &format!("invalid language tag: \"{}\"", e),
                    )))),
                },
            )
            .into_inner()
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
pub fn locale_from_environment() -> Result<Locale, String> {
    let mut locale = Locale::invariant();

    for name in glib::get_language_names() {
        let range = LanguageRange::from_unix(&name).map_err(|e| format!("{}", e))?;
        locale.add(&range);
    }

    Ok(locale)
}

fn locale_accepts_language_tag(
    locale: &Locale,
    language_tag: &LanguageTag,
) -> Result<bool, ValueErrorKind> {
    for locale_range in locale.tags_for("messages") {
        if locale_range == LanguageRange::invariant() {
            continue;
        }

        let str_locale_range = locale_range.as_ref();

        let locale_tag = LanguageTag::from_str(str_locale_range).map_err(|e| {
            ValueErrorKind::Parse(ParseError::new(&format!(
                "invalid language tag \"{}\" in locale: {}",
                str_locale_range, e
            )))
        })?;

        if !locale_tag.is_language_range() {
            return Err(ValueErrorKind::Value(format!(
                "language tag \"{}\" is not a language range",
                locale_tag
            )));
        }

        if locale_tag.matches(language_tag) {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn required_extensions() {
        assert_eq!(
            RequiredExtensions::from_attribute("http://test.org/NotExisting/1.0"),
            Ok(RequiredExtensions(false))
        );
    }

    #[test]
    fn required_features() {
        assert_eq!(
            RequiredFeatures::from_attribute("http://www.w3.org/TR/SVG11/feature#NotExisting"),
            Ok(RequiredFeatures(false))
        );

        assert_eq!(
            RequiredFeatures::from_attribute("http://www.w3.org/TR/SVG11/feature#BasicFilter"),
            Ok(RequiredFeatures(true))
        );

        assert_eq!(
            RequiredFeatures::from_attribute(
                "http://www.w3.org/TR/SVG11/feature#BasicFilter \
                 http://www.w3.org/TR/SVG11/feature#NotExisting",
            ),
            Ok(RequiredFeatures(false))
        );

        assert_eq!(
            RequiredFeatures::from_attribute(
                "http://www.w3.org/TR/SVG11/feature#BasicFilter \
                 http://www.w3.org/TR/SVG11/feature#BasicText",
            ),
            Ok(RequiredFeatures(true))
        );
    }

    #[test]
    fn system_language() {
        let user_prefers = Locale::new("de,en-US").unwrap();

        assert!(SystemLanguage::from_attribute("", &user_prefers).is_err());

        assert!(SystemLanguage::from_attribute("12345", &user_prefers).is_err());

        assert_eq!(
            SystemLanguage::from_attribute("fr", &user_prefers),
            Ok(SystemLanguage(false))
        );

        assert_eq!(
            SystemLanguage::from_attribute("en", &user_prefers),
            Ok(SystemLanguage(false))
        );

        assert_eq!(
            SystemLanguage::from_attribute("de", &user_prefers),
            Ok(SystemLanguage(true))
        );

        assert_eq!(
            SystemLanguage::from_attribute("en-US", &user_prefers),
            Ok(SystemLanguage(true))
        );

        assert_eq!(
            SystemLanguage::from_attribute("en-GB", &user_prefers),
            Ok(SystemLanguage(false))
        );

        assert_eq!(
            SystemLanguage::from_attribute("DE", &user_prefers),
            Ok(SystemLanguage(true))
        );

        assert_eq!(
            SystemLanguage::from_attribute("de-LU", &user_prefers),
            Ok(SystemLanguage(true))
        );

        assert_eq!(
            SystemLanguage::from_attribute("fr, de", &user_prefers),
            Ok(SystemLanguage(true))
        );
    }
}
