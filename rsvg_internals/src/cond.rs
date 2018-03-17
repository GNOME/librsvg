use glib;
use glib_sys;
use libc;

use error::*;
use parsers::Parse;
use std::marker::PhantomData;
use util::utf8_cstr;

use self::glib::translate::*;

// No extensions at the moment.
static IMPLEMENTED_EXTENSIONS: &[&str] = &[];

#[derive(Debug, PartialEq)]
struct RequiredExtensions(bool);

impl Parse for RequiredExtensions {
    type Data = ();
    type Err = AttributeError;

    // Parse a requiredExtensions attribute
    // http://www.w3.org/TR/SVG/struct.html#RequiredExtensionsAttribute
    fn parse(s: &str, _: ()) -> Result<RequiredExtensions, AttributeError> {
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
struct RequiredFeatures(bool);

impl Parse for RequiredFeatures {
    type Data = ();
    type Err = AttributeError;

    // Parse a requiredFeatures attribute
    // http://www.w3.org/TR/SVG/struct.html#RequiredFeaturesAttribute
    fn parse(s: &str, _: ()) -> Result<RequiredFeatures, AttributeError> {
        Ok(RequiredFeatures(
            s.split_whitespace()
                .all(|f| IMPLEMENTED_FEATURES.binary_search(&f).is_ok()),
        ))
    }
}

#[derive(Debug, PartialEq)]
struct SystemLanguage<'a>(bool, PhantomData<&'a i8>);

impl<'a> Parse for SystemLanguage<'a> {
    type Data = &'a [String];
    type Err = AttributeError;

    // Parse a systemLanguage attribute
    // http://www.w3.org/TR/SVG/struct.html#SystemLanguageAttribute
    fn parse(s: &str, system_languages: &[String]) -> Result<SystemLanguage<'a>, AttributeError> {
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

#[no_mangle]
pub extern "C" fn rsvg_cond_check_required_features(
    raw_value: *const libc::c_char,
) -> glib_sys::gboolean {
    let value = unsafe { utf8_cstr(raw_value) };

    match RequiredFeatures::parse(value, ()) {
        Ok(RequiredFeatures(res)) => res.to_glib(),
        Err(_) => false.to_glib(),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_cond_check_required_extensions(
    raw_value: *const libc::c_char,
) -> glib_sys::gboolean {
    let value = unsafe { utf8_cstr(raw_value) };

    match RequiredExtensions::parse(value, ()) {
        Ok(RequiredExtensions(res)) => res.to_glib(),
        Err(_) => false.to_glib(),
    }
}

#[no_mangle]
pub extern "C" fn rsvg_cond_check_system_language(
    raw_value: *const libc::c_char,
) -> glib_sys::gboolean {
    let value = unsafe { utf8_cstr(raw_value) };

    match SystemLanguage::parse(value, &glib::get_language_names()) {
        Ok(SystemLanguage(res, _)) => res.to_glib(),
        Err(_) => false.to_glib(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_required_features() {
        assert_eq!(
            RequiredFeatures::parse("http://www.w3.org/TR/SVG11/feature#NotExisting", ()),
            Ok(RequiredFeatures(false))
        );

        assert_eq!(
            RequiredFeatures::parse("http://www.w3.org/TR/SVG11/feature#BasicFilter", ()),
            Ok(RequiredFeatures(true))
        );

        assert_eq!(
            RequiredFeatures::parse(
                "http://www.w3.org/TR/SVG11/feature#BasicFilter
                 http://www.w3.org/TR/SVG11/feature#NotExisting",
                ()
            ),
            Ok(RequiredFeatures(false))
        );

        assert_eq!(
            RequiredFeatures::parse(
                "http://www.w3.org/TR/SVG11/feature#BasicFilter
                 http://www.w3.org/TR/SVG11/feature#BasicText",
                ()
            ),
            Ok(RequiredFeatures(true))
        );
    }

    #[test]
    fn parse_required_extensions() {
        assert_eq!(
            RequiredExtensions::parse("http://test.org/NotExisting/1.0", ()),
            Ok(RequiredExtensions(false))
        );
    }

    #[test]
    fn parse_system_language() {
        let system_languages = vec![String::from("de"), String::from("en_US")];

        assert_eq!(
            SystemLanguage::parse("", &system_languages),
            Ok(SystemLanguage(false, PhantomData))
        );

        assert_eq!(
            SystemLanguage::parse("fr", &system_languages),
            Ok(SystemLanguage(false, PhantomData))
        );

        assert_eq!(
            SystemLanguage::parse("de", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::parse("en_US", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::parse("DE", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::parse("de-LU", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );

        assert_eq!(
            SystemLanguage::parse("fr, de", &system_languages),
            Ok(SystemLanguage(true, PhantomData))
        );
    }
}
