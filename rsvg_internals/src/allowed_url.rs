//! Determine which URLs are allowed for loading.

use std::error;
use std::fmt;
use std::ops::Deref;
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

    /// Loaded file:// URLs cannot have a query part, e.g. `file:///foo?blah`
    NoQueriesAllowed,

    /// URLs may not have fragment identifiers at this stage
    NoFragmentIdentifierAllowed,

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

        // Queries are not allowed.
        if url.query().is_some() {
            return Err(AllowedUrlError::NoQueriesAllowed);
        }

        // Fragment identifiers are not allowed.  They should have been stripped
        // upstream, by NodeId.
        if url.fragment().is_some() {
            return Err(AllowedUrlError::NoFragmentIdentifierAllowed);
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

        // The rest of this function assumes file: URLs; guard against
        // incorrect refactoring.
        assert!(url.scheme() == "file");

        // If we have a base_uri of "file:///foo/bar.svg", and resolve an href of ".",
        // Url.parse() will give us "file:///foo/".  We don't want that, so check
        // if the last path segment is empty - it will not be empty for a normal file.

        if let Some(segments) = url.path_segments() {
          if segments
                .last()
                .expect("URL path segments always contain at last 1 element")
                .is_empty()
            {
                return Err(AllowedUrlError::NotSiblingOrChildOfBaseFile);
            }
        } else {
            unreachable!("the file: URL cannot have an empty path");
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

        let path_canon = url_path
            .canonicalize()
            .map_err(|_| AllowedUrlError::CanonicalizationError)?;
        let parent_canon = base_parent
            .canonicalize()
            .map_err(|_| AllowedUrlError::CanonicalizationError)?;

        if path_canon.starts_with(parent_canon) {
            // Finally, convert the canonicalized path back to a URL.
            let path_to_url = Url::from_file_path(path_canon).unwrap();
            Ok(AllowedUrl(path_to_url))
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
        use AllowedUrlError::*;
        match self {
            HrefParseError(e) => write!(f, "URL parse error: {}", e),
            BaseRequired => write!(f, "base required"),
            DifferentUriSchemes => write!(f, "different URI schemes"),
            DisallowedScheme => write!(f, "disallowed scheme"),
            NotSiblingOrChildOfBaseFile => write!(f, "not sibling or child of base file"),
            NoQueriesAllowed => write!(f, "no queries allowed"),
            NoFragmentIdentifierAllowed => write!(f, "no fragment identifier allowed"),
            InvalidPath => write!(f, "invalid path"),
            BaseIsRoot => write!(f, "base is root"),
            CanonicalizationError => write!(f, "canonicalization error"),
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

    use std::path::PathBuf;

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

    fn url_from_test_fixtures(filename_relative_to_librsvg_srcdir: &str) -> Url {
        let path = PathBuf::from(filename_relative_to_librsvg_srcdir);
        let absolute = path
            .canonicalize()
            .expect("files from test fixtures are supposed to canonicalize");
        Url::from_file_path(absolute).unwrap()
    }

    #[test]
    fn allows_relative() {
        let resolved = AllowedUrl::from_href(
            "foo.svg",
            Some(url_from_test_fixtures("../tests/fixtures/loading/bar.svg")).as_ref()
        ).unwrap();

        let resolved_str = resolved.as_str();
        assert!(resolved_str.ends_with("/loading/foo.svg"));
    }

    #[test]
    fn allows_sibling() {
        let sibling = url_from_test_fixtures("../tests/fixtures/loading/foo.svg");
        let resolved = AllowedUrl::from_href(
            sibling.as_str(),
            Some(url_from_test_fixtures("../tests/fixtures/loading/bar.svg")).as_ref()
        ).unwrap();
        
        let resolved_str = resolved.as_str();
        assert!(resolved_str.ends_with("/loading/foo.svg"));
    }

    #[test]
    fn allows_child_of_sibling() {
        let child_of_sibling = url_from_test_fixtures("../tests/fixtures/loading/subdir/baz.svg");
        let resolved = AllowedUrl::from_href(
            child_of_sibling.as_str(),
            Some(url_from_test_fixtures("../tests/fixtures/loading/bar.svg")).as_ref()
        ).unwrap();
        
        let resolved_str = resolved.as_str();
        assert!(resolved_str.ends_with("/loading/subdir/baz.svg"));
    }

    // Ignore on Windows since we test for /etc/passwd
    #[cfg(unix)]
    #[test]
    fn disallows_non_sibling() {
        assert_eq!(
            AllowedUrl::from_href(
                "file:///etc/passwd",
                Some(url_from_test_fixtures("../tests/fixtures/loading/bar.svg")).as_ref()
            ),
            Err(AllowedUrlError::NotSiblingOrChildOfBaseFile)
        );
    }

    #[test]
    fn disallows_queries() {
        match AllowedUrl::from_href(
            ".?../../../../../../../../../../etc/passwd",
            Some(url_from_test_fixtures("../tests/fixtures/loading/bar.svg")).as_ref(),
        ) {
            Err(AllowedUrlError::NoQueriesAllowed) => (),
            _ => panic!(),
        }
    }

    #[test]
    fn disallows_weird_relative_uris() {
        let base_url = url_from_test_fixtures("../tests/fixtures/loading/bar.svg");

        assert!(
            AllowedUrl::from_href(
                ".@../../../../../../../../../../etc/passwd",
                Some(&base_url),
            ).is_err()
        );
        assert!(
            AllowedUrl::from_href(
                ".$../../../../../../../../../../etc/passwd",
                Some(&base_url),
            ).is_err()
        );
        assert!(
            AllowedUrl::from_href(
                ".%../../../../../../../../../../etc/passwd",
                Some(&base_url),
            ).is_err()
        );
        assert!(
            AllowedUrl::from_href(
                ".*../../../../../../../../../../etc/passwd",
                Some(&base_url),
            ).is_err()
        );
        assert!(
            AllowedUrl::from_href(
                "~/../../../../../../../../../../etc/passwd",
                Some(&base_url),
            ).is_err()
        );
    }

    #[test]
    fn disallows_dot_sibling() {
        println!("cwd: {:?}", std::env::current_dir());
        let base_url = url_from_test_fixtures("../tests/fixtures/loading/bar.svg");

        match AllowedUrl::from_href(".", Some(&base_url)) {
            Err(AllowedUrlError::NotSiblingOrChildOfBaseFile) => (),
            _ => panic!(),
        }

        match AllowedUrl::from_href(".#../../../../../../../../../../etc/passwd", Some(&base_url)) {
            Err(AllowedUrlError::NoFragmentIdentifierAllowed) => (),
            _ => panic!(),
        }
    }

    #[test]
    fn disallows_fragment() {
        // AllowedUrl::from_href() explicitly disallows fragment identifiers.
        // This is because they should have been stripped before calling that function,
        // by the Iri machinery.

        match AllowedUrl::from_href("bar.svg#fragment", Some(Url::parse("https://example.com/foo.svg").unwrap()).as_ref()) {
            Err(AllowedUrlError::NoFragmentIdentifierAllowed) => (),
            _ => panic!(),
        }
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
