use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use allowed_url::AllowedUrl;
use error::ValueErrorKind;
use handle::{self, RsvgHandle};
use node::Node;
use parsers::ParseError;

pub struct Defs {
    externs: HashMap<AllowedUrl, *const RsvgHandle>,
}

impl Defs {
    pub fn new() -> Defs {
        Defs {
            externs: Default::default(),
        }
    }

    /// Returns a node referenced by a fragment ID, from an
    /// externally-loaded SVG file.
    pub fn lookup(&mut self, handle: *const RsvgHandle, fragment: &Fragment) -> Option<Rc<Node>> {
        if let Some(ref href) = fragment.uri() {
            match self.get_extern_handle(handle, href) {
                Ok(extern_handle) => handle::lookup_fragment_id(extern_handle, fragment.fragment()),
                Err(()) => None,
            }
        } else {
            unreachable!();
        }
    }

    fn get_extern_handle(
        &mut self,
        handle: *const RsvgHandle,
        href: &str,
    ) -> Result<*const RsvgHandle, ()> {
        let aurl =
            AllowedUrl::from_href(href, handle::get_base_url(handle).as_ref()).map_err(|_| ())?;

        match self.externs.entry(aurl) {
            Entry::Occupied(e) => Ok(*(e.get())),
            Entry::Vacant(e) => {
                let load_options = handle::get_load_options(handle);
                let extern_handle = handle::load_extern(&load_options, e.key())?;
                e.insert(extern_handle);
                Ok(extern_handle)
            }
        }
    }
}

/// Parsed result of an href from an SVG or CSS file
///
/// Sometimes in SVG element references (e.g. the `href` in the `<feImage>` element) we
/// must decide between referencing an external file, or using a plain fragment identifier
/// like `href="#foo"` as a reference to an SVG element in the same file as the one being
/// processed.  This enum makes that distinction.
#[derive(Debug, PartialEq)]
pub enum Href {
    PlainUri(String),
    WithFragment(Fragment),
}

/// Optional URI, mandatory fragment id
#[derive(Debug, PartialEq, Clone)]
pub struct Fragment(Option<String>, String);

impl Fragment {
    // Outside of testing, we don't want code creating Fragments by hand;
    // they should get them from Href.
    #[cfg(test)]
    pub fn new(uri: Option<String>, fragment: String) -> Fragment {
        Fragment(uri, fragment)
    }

    pub fn parse(href: &str) -> Result<Fragment, HrefError> {
        let href = Href::with_fragment(href)?;

        if let Href::WithFragment(f) = href {
            Ok(f)
        } else {
            unreachable!();
        }
    }

    pub fn uri(&self) -> Option<&str> {
        self.0.as_ref().map(|s| s.as_str())
    }

    pub fn fragment(&self) -> &str {
        &self.1
    }
}

impl fmt::Display for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}#{}",
            self.0.as_ref().map(String::as_str).unwrap_or(""),
            self.1
        )
    }
}

/// Errors returned when creating an `Href` out of a string
#[derive(Debug, Clone, PartialEq)]
pub enum HrefError {
    /// The href is an invalid URI or has empty components.
    ParseError,

    /// A fragment identifier ("`#foo`") is not allowed here
    ///
    /// For example, the SVG `<image>` element only allows referencing
    /// resources without fragment identifiers like
    /// `xlink:href="foo.png"`.
    FragmentForbidden,

    /// A fragment identifier ("`#foo`") was required but not found.  For example,
    /// the SVG `<use>` element requires one, as in `<use xlink:href="foo.svg#bar">`.
    FragmentRequired,
}

impl From<HrefError> for ValueErrorKind {
    fn from(e: HrefError) -> ValueErrorKind {
        match e {
            HrefError::ParseError => ValueErrorKind::Parse(ParseError::new("url parse error")),
            HrefError::FragmentForbidden => {
                ValueErrorKind::Value("fragment identifier not allowed".to_string())
            }
            HrefError::FragmentRequired => {
                ValueErrorKind::Value("fragment identifier required".to_string())
            }
        }
    }
}

impl Href {
    /// Parses a string into an Href, or returns an error
    ///
    /// An href can come from an `xlink:href` attribute in an SVG
    /// element.  This function determines if the provided href is a
    /// plain absolute or relative URL ("`foo.png`"), or one with a
    /// fragment identifier ("`foo.svg#bar`").
    pub fn parse(href: &str) -> Result<Href, HrefError> {
        let (uri, fragment) = match href.rfind('#') {
            None => (Some(href), None),
            Some(p) if p == 0 => (None, Some(&href[1..])),
            Some(p) => (Some(&href[..p]), Some(&href[(p + 1)..])),
        };

        match (uri, fragment) {
            (None, Some(f)) if f.len() == 0 => Err(HrefError::ParseError),
            (None, Some(f)) => Ok(Href::WithFragment(Fragment(None, f.to_string()))),
            (Some(u), _) if u.len() == 0 => Err(HrefError::ParseError),
            (Some(u), None) => Ok(Href::PlainUri(u.to_string())),
            (Some(_u), Some(f)) if f.len() == 0 => Err(HrefError::ParseError),
            (Some(u), Some(f)) => Ok(Href::WithFragment(Fragment(
                Some(u.to_string()),
                f.to_string(),
            ))),
            (_, _) => Err(HrefError::ParseError),
        }
    }

    pub fn without_fragment(href: &str) -> Result<Href, HrefError> {
        use self::Href::*;

        match Href::parse(href)? {
            r @ PlainUri(_) => Ok(r),
            WithFragment(_) => Err(HrefError::FragmentForbidden),
        }
    }

    pub fn with_fragment(href: &str) -> Result<Href, HrefError> {
        use self::Href::*;

        match Href::parse(href)? {
            PlainUri(_) => Err(HrefError::FragmentRequired),
            r @ WithFragment(_) => Ok(r),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        assert_eq!(
            Href::parse("uri").unwrap(),
            Href::PlainUri("uri".to_string())
        );
        assert_eq!(
            Href::parse("#fragment").unwrap(),
            Href::WithFragment(Fragment::new(None, "fragment".to_string()))
        );
        assert_eq!(
            Href::parse("uri#fragment").unwrap(),
            Href::WithFragment(Fragment::new(
                Some("uri".to_string()),
                "fragment".to_string()
            ))
        );
    }

    #[test]
    fn parse_errors() {
        assert_eq!(Href::parse(""), Err(HrefError::ParseError));
        assert_eq!(Href::parse("#"), Err(HrefError::ParseError));
        assert_eq!(Href::parse("uri#"), Err(HrefError::ParseError));
    }

    #[test]
    fn without_fragment() {
        assert_eq!(
            Href::without_fragment("uri").unwrap(),
            Href::PlainUri("uri".to_string())
        );

        assert_eq!(
            Href::without_fragment("#foo"),
            Err(HrefError::FragmentForbidden)
        );

        assert_eq!(
            Href::without_fragment("uri#foo"),
            Err(HrefError::FragmentForbidden)
        );
    }

    #[test]
    fn with_fragment() {
        assert_eq!(
            Href::with_fragment("#foo").unwrap(),
            Href::WithFragment(Fragment::new(None, "foo".to_string()))
        );

        assert_eq!(
            Href::with_fragment("uri#foo").unwrap(),
            Href::WithFragment(Fragment::new(Some("uri".to_string()), "foo".to_string()))
        );

        assert_eq!(Href::with_fragment("uri"), Err(HrefError::FragmentRequired));
    }

    #[test]
    fn fragment_parse() {
        assert_eq!(
            Fragment::parse("#foo").unwrap(),
            Fragment::new(None, "foo".to_string())
        );

        assert_eq!(
            Fragment::parse("uri#foo").unwrap(),
            Fragment::new(Some("uri".to_string()), "foo".to_string())
        );

        assert_eq!(Fragment::parse("uri"), Err(HrefError::FragmentRequired));
    }
}
