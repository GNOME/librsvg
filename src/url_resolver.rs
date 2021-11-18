//! Determine which URLs are allowed for loading.

use std::fmt;
use std::io;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use url::Url;

use crate::error::AllowedUrlError;

/// Currently only contains the base URL.
///
/// The plan is to add:
/// base_only:    Only allow to load content from the same base URL. By default
//                this restriction is enabled and requires to provide base_url.
/// include_xml:  Allows to use xi:include with XML. Enabled by default.
/// include_text: Allows to use xi:include with text. Enabled by default.
/// local_only:   Only allow to load content from the local filesystem.
///               Enabled by default.
#[derive(Clone)]
pub struct UrlResolver {
    /// Base URL; all relative references will be resolved with respect to this.
    pub base_url: Option<Url>,
}

impl UrlResolver {
    /// Creates a `UrlResolver` with defaults, and sets the `base_url`.
    pub fn new(base_url: Option<Url>) -> Self {
        UrlResolver { base_url }
    }

    pub fn resolve_href(&self, href: &str) -> Result<AllowedUrl, AllowedUrlError> {
        let url = Url::options()
            .base_url(self.base_url.as_ref())
            .parse(href)
            .map_err(AllowedUrlError::UrlParseError)?;

        // Allow loads of data: from any location
        if url.scheme() == "data" {
            return Ok(AllowedUrl(url));
        }

        // All other sources require a base url
        if self.base_url.is_none() {
            return Err(AllowedUrlError::BaseRequired);
        }

        let base_url = self.base_url.as_ref().unwrap();

        // Deny loads from differing URI schemes
        if url.scheme() != base_url.scheme() {
            return Err(AllowedUrlError::DifferentUriSchemes);
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

/// Wrapper for URLs which are allowed to be loaded
///
/// SVG files can reference other files (PNG/JPEG images, other SVGs,
/// CSS files, etc.).  This object is constructed by checking whether
/// a specified `href` (a possibly-relative filename, for example)
/// should be allowed to be loaded, given the base URL of the SVG
/// being loaded.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AllowedUrl(Url);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disallows_relative_file_with_no_base_file() {
        let url_resolver = UrlResolver::new(None);
        assert!(matches!(
            url_resolver.resolve_href("foo.svg"),
            Err(AllowedUrlError::UrlParseError(
                url::ParseError::RelativeUrlWithoutBase
            ))
        ));
    }

    #[test]
    fn disallows_different_schemes() {
        let url_resolver = UrlResolver::new(Some(
            Url::parse("http://example.com/malicious.svg").unwrap(),
        ));
        assert!(matches!(
            url_resolver.resolve_href("file:///etc/passwd"),
            Err(AllowedUrlError::DifferentUriSchemes)
        ));
    }

    fn make_file_uri(p: &str) -> String {
        if cfg!(windows) {
            format!("file:///c:{}", p)
        } else {
            format!("file://{}", p)
        }
    }

    #[test]
    fn disallows_base_is_root() {
        let url_resolver = UrlResolver::new(Some(Url::parse(&make_file_uri("/")).unwrap()));
        assert!(matches!(
            url_resolver.resolve_href("foo.svg"),
            Err(AllowedUrlError::BaseIsRoot)
        ));
    }

    #[test]
    fn disallows_non_file_scheme() {
        let url_resolver = UrlResolver::new(Some(Url::parse("http://foo.bar/baz.svg").unwrap()));
        assert!(matches!(
            url_resolver.resolve_href("foo.svg"),
            Err(AllowedUrlError::DisallowedScheme)
        ));
    }

    #[test]
    fn allows_data_url_with_no_base_file() {
        let url_resolver = UrlResolver::new(None);
        assert_eq!(
            url_resolver
                .resolve_href("data:image/jpeg;base64,xxyyzz")
                .unwrap()
                .as_ref(),
            "data:image/jpeg;base64,xxyyzz",
        );
    }

    #[test]
    fn allows_relative() {
        let url_resolver = UrlResolver::new(Some(
            Url::parse(&make_file_uri("/example/bar.svg")).unwrap(),
        ));
        let resolved = url_resolver.resolve_href("foo.svg").unwrap();
        let expected = make_file_uri("/example/foo.svg");
        assert_eq!(resolved.as_ref(), expected);
    }

    #[test]
    fn allows_sibling() {
        let url_resolver = UrlResolver::new(Some(
            Url::parse(&make_file_uri("/example/bar.svg")).unwrap(),
        ));
        let resolved = url_resolver
            .resolve_href(&make_file_uri("/example/foo.svg"))
            .unwrap();
        let expected = make_file_uri("/example/foo.svg");
        assert_eq!(resolved.as_ref(), expected);
    }

    #[test]
    fn allows_child_of_sibling() {
        let url_resolver = UrlResolver::new(Some(
            Url::parse(&make_file_uri("/example/bar.svg")).unwrap(),
        ));
        let resolved = url_resolver
            .resolve_href(&make_file_uri("/example/subdir/foo.svg"))
            .unwrap();
        let expected = make_file_uri("/example/subdir/foo.svg");
        assert_eq!(resolved.as_ref(), expected);
    }

    #[test]
    fn disallows_non_sibling() {
        let url_resolver = UrlResolver::new(Some(
            Url::parse(&make_file_uri("/example/bar.svg")).unwrap(),
        ));
        assert!(matches!(
            url_resolver.resolve_href(&make_file_uri("/etc/passwd")),
            Err(AllowedUrlError::NotSiblingOrChildOfBaseFile)
        ));
    }
}
