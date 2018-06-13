use error::*;
use std::marker::PhantomData;

#[allow(unused_imports, deprecated)]
use std::ascii::AsciiExt;

// No extensions at the moment.
static IMPLEMENTED_EXTENSIONS: &[&str] = &[];

#[derive(Debug, PartialEq)]
pub struct RequiredExtensions(pub bool);

impl RequiredExtensions {
    // Parse a requiredExtensions attribute
    // http://www.w3.org/TR/SVG/struct.html#RequiredExtensionsAttribute
    pub fn from_attribute(s: &str) -> Result<RequiredExtensions, AttributeError> {
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
    pub fn from_attribute(s: &str) -> Result<RequiredFeatures, AttributeError> {
        Ok(RequiredFeatures(
            s.split_whitespace()
                .all(|f| IMPLEMENTED_FEATURES.binary_search(&f).is_ok()),
        ))
    }
}

#[derive(Debug, PartialEq)]
pub struct SystemLanguage<'a>(pub bool, pub PhantomData<&'a i8>);

impl<'a> SystemLanguage<'a> {
    // Parse a systemLanguage attribute
    // http://www.w3.org/TR/SVG/struct.html#SystemLanguageAttribute
    pub fn from_attribute(
        s: &str,
        system_languages: &[String],
    ) -> Result<SystemLanguage<'a>, AttributeError> {
        Ok(SystemLanguage(
            s.split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .any(|l| {
                    system_languages.iter().any(|sl| {
                        if sl.eq_ignore_ascii_case(l) {
                            return true;
                        }

                        if let Some(offset) = l.find('-') {
                            return sl.eq_ignore_ascii_case(&l[..offset]);
                        }

                        false
                    })
                }),
            PhantomData,
        ))
    }
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
        let system_languages = vec![String::from("de"), String::from("en_US")];

        assert_eq!(
            SystemLanguage::from_attribute("", &system_languages),
            Ok(SystemLanguage(false, PhantomData))
        );

        assert_eq!(
            SystemLanguage::from_attribute("fr", &system_languages),
            Ok(SystemLanguage(false, PhantomData))
        );

        assert_eq!(
            SystemLanguage::from_attribute("de", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::from_attribute("en_US", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::from_attribute("DE", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::from_attribute("de-LU", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::from_attribute("fr, de", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );
    }
}
