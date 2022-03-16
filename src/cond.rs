//! Conditional processing attributes: `requiredExtensions`, `requiredFeatures`, `systemLanguage`.

#[allow(unused_imports, deprecated)]
use std::ascii::AsciiExt;

use std::str::FromStr;

use language_tags::LanguageTag;

use crate::accept_language::{LanguageTags, UserLanguage};
use crate::error::*;

// No extensions at the moment.
static IMPLEMENTED_EXTENSIONS: &[&str] = &[];

#[derive(Debug, PartialEq)]
pub struct RequiredExtensions(pub bool);

impl RequiredExtensions {
    /// Parse a requiredExtensions attribute.
    ///
    /// <http://www.w3.org/TR/SVG/struct.html#RequiredExtensionsAttribute>
    pub fn from_attribute(s: &str) -> Result<RequiredExtensions, ValueErrorKind> {
        Ok(RequiredExtensions(
            s.split_whitespace()
                .all(|f| IMPLEMENTED_EXTENSIONS.binary_search(&f).is_ok()),
        ))
    }

    /// Evaluate a requiredExtensions value for conditional processing.
    pub fn eval(&self) -> bool {
        self.0
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

    /// Evaluate a requiredFeatures value for conditional processing.
    pub fn eval(&self) -> bool {
        self.0
    }
}

#[derive(Debug, PartialEq)]
pub struct SystemLanguage(LanguageTags);

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
    pub fn from_attribute(s: &str) -> Result<SystemLanguage, ValueErrorKind> {
        let attribute_tags = s
            .split(',')
            .map(str::trim)
            .map(|s| {
                LanguageTag::from_str(s).map_err(|e| {
                    ValueErrorKind::parse_error(&format!("invalid language tag: \"{}\"", e))
                })
            })
            .collect::<Result<Vec<LanguageTag>, _>>()?;

        Ok(SystemLanguage(LanguageTags::from(attribute_tags)))
    }

    /// Evaluate a systemLanguage value for conditional processing.
    pub fn eval(&self, user_language: &UserLanguage) -> bool {
        user_language.any_matches(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use locale_config::Locale;

    #[test]
    fn required_extensions() {
        assert_eq!(
            RequiredExtensions::from_attribute("http://test.org/NotExisting/1.0").unwrap(),
            RequiredExtensions(false)
        );
    }

    #[test]
    fn required_features() {
        assert_eq!(
            RequiredFeatures::from_attribute("http://www.w3.org/TR/SVG11/feature#NotExisting")
                .unwrap(),
            RequiredFeatures(false)
        );

        assert_eq!(
            RequiredFeatures::from_attribute("http://www.w3.org/TR/SVG11/feature#BasicFilter")
                .unwrap(),
            RequiredFeatures(true)
        );

        assert_eq!(
            RequiredFeatures::from_attribute(
                "http://www.w3.org/TR/SVG11/feature#BasicFilter \
                 http://www.w3.org/TR/SVG11/feature#NotExisting",
            )
            .unwrap(),
            RequiredFeatures(false)
        );

        assert_eq!(
            RequiredFeatures::from_attribute(
                "http://www.w3.org/TR/SVG11/feature#BasicFilter \
                 http://www.w3.org/TR/SVG11/feature#BasicText",
            )
            .unwrap(),
            RequiredFeatures(true)
        );
    }

    #[test]
    fn system_language() {
        let locale = Locale::new("de,en-US").unwrap();
        let user_language = UserLanguage::LanguageTags(LanguageTags::from_locale(&locale).unwrap());

        assert!(SystemLanguage::from_attribute("").is_err());

        assert!(SystemLanguage::from_attribute("12345").is_err());

        assert_eq!(
            SystemLanguage::from_attribute("fr")
                .unwrap()
                .eval(&user_language),
            false
        );

        assert_eq!(
            SystemLanguage::from_attribute("en")
                .unwrap()
                .eval(&user_language),
            false
        );

        assert_eq!(
            SystemLanguage::from_attribute("de")
                .unwrap()
                .eval(&user_language),
            true
        );

        assert_eq!(
            SystemLanguage::from_attribute("en-US")
                .unwrap()
                .eval(&user_language),
            true
        );

        assert_eq!(
            SystemLanguage::from_attribute("en-GB")
                .unwrap()
                .eval(&user_language),
            false
        );

        assert_eq!(
            SystemLanguage::from_attribute("DE")
                .unwrap()
                .eval(&user_language),
            true
        );

        assert_eq!(
            SystemLanguage::from_attribute("de-LU")
                .unwrap()
                .eval(&user_language),
            true
        );

        assert_eq!(
            SystemLanguage::from_attribute("fr, de")
                .unwrap()
                .eval(&user_language),
            true
        );
    }
}
