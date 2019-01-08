use std::error::{self, Error};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use url::{self, Url};

/// Wrapper for URLs which are allowed to be loaded
///
/// SVG files can reference other files (PNG/JPEG images, other SVGs,
/// CSS files, etc.).  This object is constructed by checking whether
/// a specified `href` (a possibly-relative filename, for example)
/// should be allowed to be loaded, given the base URL of the SVG
/// being loaded.
#[derive(Debug, PartialEq, Eq, Hash)]
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

    pub fn url(&self) -> &Url {
        &self.0
    }
}

impl fmt::Display for AllowedUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl error::Error for AllowedUrlError {
    fn description(&self) -> &str {
        match *self {
            AllowedUrlError::HrefParseError(_) => "href parse error",
            AllowedUrlError::BaseRequired => "base required",
            AllowedUrlError::DifferentURISchemes => "different URI schemes",
            AllowedUrlError::DisallowedScheme => "disallowed scheme",
            AllowedUrlError::NotSiblingOrChildOfBaseFile => "not sibling or child of base file",
            AllowedUrlError::InvalidPath => "invalid path",
            AllowedUrlError::BaseIsRoot => "base is root",
            AllowedUrlError::CanonicalizationError => "canonicalization error",
        }
    }
}

impl fmt::Display for AllowedUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            AllowedUrlError::HrefParseError(e) => write!(f, "{}: {}", self.description(), e),
            _ => write!(f, "{}", self.description()),
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
                .url(),
            &Url::parse("data:image/jpeg;base64,xxyyzz").unwrap(),
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
            .url(),
            &Url::parse("file:///example/foo.svg").unwrap(),
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
            .url(),
            &Url::parse("file:///example/foo.svg").unwrap(),
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
            .url(),
            &Url::parse("file:///example/subdir/foo.svg").unwrap(),
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
}
