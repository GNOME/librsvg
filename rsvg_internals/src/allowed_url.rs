//! Determine which URLs are allowed for loading.

use std::error;
use std::fmt;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use url::Url;

use crate::error::HrefError;

/// Wrapper for URLs which are allowed to be loaded
///
/// SVG files can reference other files (PNG/JPEG images, other SVGs,
/// CSS files, etc.).  This object is constructed by checking whether
/// a specified `href` (a possibly-relative filename, for example)
/// should be allowed to be loaded, given the base URL of the SVG
/// being loaded.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AllowedUrl(Url);

#[derive(Debug, PartialEq)]
pub enum AllowedUrlError {
    /// parsing error from `Url::parse()`
    HrefParseError(url::ParseError),

    /// A base file/uri was not set
    BaseRequired,

    /// Cannot reference a file with a different URI scheme from the base file
    DifferentURISchemes,

    /// Some scheme we don't allow loading
    DisallowedScheme,

    /// The requested file is not in the same directory as the base file,
    /// or in one directory below the base file.
    NotSiblingOrChildOfBaseFile,

    /// Error when obtaining the file path or the base file path
    InvalidPath,

    /// The base file cannot be the root of the file system
    BaseIsRoot,

    /// Error when canonicalizing either the file path or the base file path
    CanonicalizationError,
}

impl AllowedUrl {
    pub fn from_href(href: &str, base_url: Option<&Url>) -> Result<AllowedUrl, AllowedUrlError> {
        let url = Url::options()
            .base_url(base_url)
            .parse(href)
            .map_err(AllowedUrlError::HrefParseError)?;

        // Allow loads of data: from any location
        if url.scheme() == "data" {
            return Ok(AllowedUrl(url));
        }

        // All other sources require a base url
        if base_url.is_none() {
            return Err(AllowedUrlError::BaseRequired);
        }

        let base_url = base_url.unwrap();

        // Deny loads from differing URI schemes
        if url.scheme() != base_url.scheme() {
            return Err(AllowedUrlError::DifferentURISchemes);
        }

        // resource: is allowed to load anything from other resources
        if url.scheme() == "resource" {
            return Ok(AllowedUrl(url));
        }

        // Non-file: isn't allowed to load anything
        if url.scheme() != "file" {
            return Err(AllowedUrlError::DisallowedScheme);
        }

        // We have two file: URIs.  Now canonicalize them (remove .. and symlinks, etc.)
        // and see if the directories match

        let url_path = url
            .to_file_path()
            .map_err(|_| AllowedUrlError::InvalidPath)?;
        let base_path = base_url
            .to_file_path()
            .map_err(|_| AllowedUrlError::InvalidPath)?;

        let base_parent = base_path.parent();
        if base_parent.is_none() {
            return Err(AllowedUrlError::BaseIsRoot);
        }

        let base_parent = base_parent.unwrap();

        let url_canon =
            canonicalize(&url_path).map_err(|_| AllowedUrlError::CanonicalizationError)?;
        let parent_canon =
            canonicalize(&base_parent).map_err(|_| AllowedUrlError::CanonicalizationError)?;

        if url_canon.starts_with(parent_canon) {
            Ok(AllowedUrl(url))
        } else {
            Err(AllowedUrlError::NotSiblingOrChildOfBaseFile)
        }
    }
}

impl Deref for AllowedUrl {
    type Target = Url;

    fn deref(&self) -> &Url {
        &self.0
    }
}

impl fmt::Display for AllowedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl error::Error for AllowedUrlError {}

impl fmt::Display for AllowedUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            AllowedUrlError::HrefParseError(e) => write!(f, "href parse error: {}", e),
            AllowedUrlError::BaseRequired => write!(f, "base required"),
            AllowedUrlError::DifferentURISchemes => write!(f, "different URI schemes"),
            AllowedUrlError::DisallowedScheme => write!(f, "disallowed scheme"),
            AllowedUrlError::NotSiblingOrChildOfBaseFile => {
                write!(f, "not sibling or child of base file")
            }
            AllowedUrlError::InvalidPath => write!(f, "invalid path"),
            AllowedUrlError::BaseIsRoot => write!(f, "base is root"),
            AllowedUrlError::CanonicalizationError => write!(f, "canonicalization error"),
        }
    }
}

// For tests, we don't want to touch the filesystem.  In that case,
// assume that we are being passed canonical file names.
#[cfg(not(test))]
fn canonicalize<P: AsRef<Path>>(path: P) -> Result<PathBuf, io::Error> {
    path.as_ref().canonicalize()
}
#[cfg(test)]
fn canonicalize<P: AsRef<Path>>(path: P) -> Result<PathBuf, io::Error> {
    Ok(path.as_ref().to_path_buf())
}

/// Parsed result of an href from an SVG or CSS file
///
/// Sometimes in SVG element references (e.g. the `href` in the `<feImage>` element) we
/// must decide between referencing an external file, or using a plain fragment identifier
/// like `href="#foo"` as a reference to an SVG element in the same file as the one being
/// processed.  This enum makes that distinction.
#[derive(Debug, PartialEq)]
pub enum Href {
    PlainUrl(String),
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
        match Href::parse(&href)? {
            Href::PlainUrl(_) => Err(HrefError::FragmentRequired),
            Href::WithFragment(f) => Ok(f),
        }
    }

    pub fn uri(&self) -> Option<&str> {
        self.0.as_deref()
    }

    pub fn fragment(&self) -> &str {
        &self.1
    }
}

impl fmt::Display for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}#{}", self.uri().unwrap_or(""), self.fragment())
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
            (None, Some(f)) if f.is_empty() => Err(HrefError::ParseError),
            (None, Some(f)) => Ok(Href::WithFragment(Fragment(None, f.to_string()))),
            (Some(u), _) if u.is_empty() => Err(HrefError::ParseError),
            (Some(u), None) => Ok(Href::PlainUrl(u.to_string())),
            (Some(_u), Some(f)) if f.is_empty() => Err(HrefError::ParseError),
            (Some(u), Some(f)) => Ok(Href::WithFragment(Fragment(
                Some(u.to_string()),
                f.to_string(),
            ))),
            (_, _) => Err(HrefError::ParseError),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disallows_relative_file_with_no_base_file() {
        assert_eq!(
            AllowedUrl::from_href("foo.svg", None),
            Err(AllowedUrlError::HrefParseError(
                url::ParseError::RelativeUrlWithoutBase
            ))
        );
    }

    #[test]
    fn disallows_different_schemes() {
        assert_eq!(
            AllowedUrl::from_href(
                "file:///etc/passwd",
                Some(Url::parse("http://example.com/malicious.svg").unwrap()).as_ref()
            ),
            Err(AllowedUrlError::DifferentURISchemes)
        );
    }

    #[test]
    fn disallows_base_is_root() {
        assert_eq!(
            AllowedUrl::from_href("foo.svg", Some(Url::parse("file:///").unwrap()).as_ref()),
            Err(AllowedUrlError::BaseIsRoot)
        );
    }

    #[test]
    fn disallows_non_file_scheme() {
        assert_eq!(
            AllowedUrl::from_href(
                "foo.svg",
                Some(Url::parse("http://foo.bar/baz.svg").unwrap()).as_ref()
            ),
            Err(AllowedUrlError::DisallowedScheme)
        );
    }

    #[test]
    fn allows_data_url_with_no_base_file() {
        assert_eq!(
            AllowedUrl::from_href("data:image/jpeg;base64,xxyyzz", None)
                .unwrap()
                .as_ref(),
            "data:image/jpeg;base64,xxyyzz",
        );
    }

    #[test]
    fn allows_relative() {
        assert_eq!(
            AllowedUrl::from_href(
                "foo.svg",
                Some(Url::parse("file:///example/bar.svg").unwrap()).as_ref()
            )
            .unwrap()
            .as_ref(),
            "file:///example/foo.svg",
        );
    }

    #[test]
    fn allows_sibling() {
        assert_eq!(
            AllowedUrl::from_href(
                "file:///example/foo.svg",
                Some(Url::parse("file:///example/bar.svg").unwrap()).as_ref()
            )
            .unwrap()
            .as_ref(),
            "file:///example/foo.svg",
        );
    }

    #[test]
    fn allows_child_of_sibling() {
        assert_eq!(
            AllowedUrl::from_href(
                "file:///example/subdir/foo.svg",
                Some(Url::parse("file:///example/bar.svg").unwrap()).as_ref()
            )
            .unwrap()
            .as_ref(),
            "file:///example/subdir/foo.svg",
        );
    }

    #[test]
    fn disallows_non_sibling() {
        assert_eq!(
            AllowedUrl::from_href(
                "file:///etc/passwd",
                Some(Url::parse("file:///example/bar.svg").unwrap()).as_ref()
            ),
            Err(AllowedUrlError::NotSiblingOrChildOfBaseFile)
        );
    }

    #[test]
    fn parses_href() {
        assert_eq!(
            Href::parse("uri").unwrap(),
            Href::PlainUrl("uri".to_string())
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

        assert_eq!(Href::parse(""), Err(HrefError::ParseError));
        assert_eq!(Href::parse("#"), Err(HrefError::ParseError));
        assert_eq!(Href::parse("uri#"), Err(HrefError::ParseError));
    }

    #[test]
    fn parses_fragment() {
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
