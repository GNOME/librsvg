//! Load and render SVG images into Cairo surfaces.
//!
//! This crate can load SVG images and render them to Cairo surfaces,
//! using a mixture of SVG's [static mode] and [secure static mode].
//! Librsvg does not do animation nor scripting, and can load
//! references to external data only in some situations; see below.
//!
//! Librsvg supports reading [SVG 1.1] data, and is gradually adding
//! support for features in [SVG 2].  Librsvg also supports SVGZ
//! files, which are just an SVG stream compressed with the GZIP
//! algorithm.
//!
//! # Basic usage
//!
//! * Create a [`Loader`] struct.
//! * Get an [`SvgHandle`] from the [`Loader`].
//! * Create a [`CairoRenderer`] for the [`SvgHandle`] and render to a Cairo context.
//!
//! You can put the following in your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! librsvg = { git="https://gitlab.gnome.org/GNOME/librsvg" }
//! cairo-rs = "0.8.0"
//! glib = "0.9.0"                                # only if you need streams
//! gio = { version="0.8.0", features=["v2_50"] } # likewise
//! ```
//!
//! # Example
//!
//! ```
//!
//! const WIDTH: i32 = 640;
//! const HEIGHT: i32 = 480;
//!
//! fn main() {
//!     // Loading from a file
//!
//!     let handle = librsvg::Loader::new().read_path("example.svg").unwrap();
//!
//!     let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, WIDTH, HEIGHT).unwrap();
//!     let cr = cairo::Context::new(&surface);
//!
//!     let renderer = librsvg::CairoRenderer::new(&handle);
//!     renderer.render_document(
//!         &cr,
//!         &cairo::Rectangle {
//!             x: 0.0,
//!             y: 0.0,
//!             width: f64::from(WIDTH),
//!             height: f64::from(HEIGHT),
//!         },
//!     ).unwrap();
//!
//!     // Loading from a static SVG asset
//!
//!     let bytes = glib::Bytes::from_static(
//!         br#"<?xml version="1.0" encoding="UTF-8"?>
//!             <svg xmlns="http://www.w3.org/2000/svg" width="50" height="50">
//!                 <rect id="foo" x="10" y="10" width="30" height="30"/>
//!             </svg>
//!         "#
//!     );
//!     let stream = gio::MemoryInputStream::new_from_bytes(&bytes);
//!
//!     let handle = librsvg::Loader::new().read_stream(
//!         &stream,
//!         None::<&gio::File>,          // no base file as this document has no references
//!         None::<&gio::Cancellable>,   // no cancellable
//!     ).unwrap();
//! }
//! ```
//!
//! [`Loader`]: struct.Loader.html
//! [`SvgHandle`]: struct.SvgHandle.html
//! [`CairoRenderer`]: struct.CairoRenderer.html
//!
//! # The "base file" and resolving references to external files
//!
//! When you load an SVG, librsvg needs to know the location of the "base file"
//! for it.  This is so that librsvg can determine the location of referenced
//! entities.  For example, say you have an SVG in <filename>/foo/bar/foo.svg</filename>
//! and that it has an image element like this:
//!
//! ```xml
//! <image href="resources/foo.png" .../>
//! ```
//!
//! In this case, librsvg needs to know the location of the toplevel
//! `/foo/bar/foo.svg` so that it can generate the appropriate
//! reference to `/foo/bar/resources/foo.png`.
//!
//! ## Security and locations of referenced files
//!
//! When processing an SVG, librsvg will only load referenced files if
//! they are in the same directory as the base file, or in a
//! subdirectory of it.  That is, if the base file is
//! `/foo/bar/baz.svg`, then librsvg will only try to load referenced
//! files (from SVG's `<image>` element, for example, or from content
//! included through XML entities) if those files are in `/foo/bar/*`
//! or in `/foo/bar/*/.../*`.  This is so that malicious SVG documents
//! cannot include files that are in a directory above.
//!
//! The full set of rules for deciding which URLs may be loaded is as follows;
//! they are applied in order.  A referenced URL will not be loaded as soon as
//! one of these rules fails:
//!
//! 1. All `data:` URLs may be loaded.  These are sometimes used to
//! include raster image data, encoded as base-64, directly in an SVG
//! file.
//!
//! 2. All other URL schemes in references require a base URL.  For
//! example, this means that if you load an SVG with
//! [`Loader.read`](struct.Loader.html#method.read) without
//! providing a `base_url`, then any referenced files will not be
//! allowed (e.g. raster images to be loaded from other files will not
//! work).
//!
//! 3. If referenced URLs are absolute, rather than relative, then
//! they must have the same scheme as the base URL.  For example, if
//! the base URL has a "`file`" scheme, then all URL references inside
//! the SVG must also have the "`file`" scheme, or be relative
//! references which will be resolved against the base URL.
//!
//! 4. If referenced URLs have a "`resource`" scheme, that is, if they
//! are included into your binary program with GLib's resource
//! mechanism, they are allowed to be loaded (provided that the base
//! URL is also a "`resource`", per the previous rule).
//!
//! 5. Otherwise, non-`file` schemes are not allowed.  For example,
//! librsvg will not load `http` resources, to keep malicious SVG data
//! from "phoning home".
//!
//! 6. A relative URL must resolve to the same directory as the base
//! URL, or to one of its subdirectories.  Librsvg will canonicalize
//! filenames, by removing "`..`" path components and resolving symbolic
//! links, to decide whether files meet these conditions.
//!
//! [static mode]: https://www.w3.org/TR/SVG2/conform.html#static-mode
//! [secure static mode]: https://www.w3.org/TR/SVG2/conform.html#secure-static-mode
//! [SVG 1.1]: https://www.w3.org/TR/SVG11/
//! [SVG 2]: https://www.w3.org/TR/SVG2/

// Enable lint group collections
#![warn(nonstandard_style, rust_2018_idioms, unused)]
// Some lints no longer exist
#![warn(renamed_and_removed_lints)]
// Standalone lints
#![warn(trivial_casts, trivial_numeric_casts)]
#![warn(missing_docs)]
#![deny(warnings)]

use glib::prelude::*;
use url::Url;

use std::path::Path;

use gio::{Cancellable, FileExt};

use rsvg_internals::{Dpi, Handle, LoadOptions};

pub use rsvg_internals::{
    DefsLookupErrorKind, HrefError, Length as InternalLength, LengthUnit, LoadingError,
    RenderingError, RsvgLength as Length,
};

/// Builder for loading an [`SvgHandle`][SvgHandle].
///
/// This is the starting point for using librsvg.  This struct
/// implements a builder pattern for configuring an
/// [`SvgHandle`][SvgHandle]'s options, and then loading the SVG data.
/// You can call the methods of `Loader` in sequence to configure
/// how SVG data should be loaded, and finally use one of the loading
/// functions to load an [`SvgHandle`][SvgHandle].
///
/// [SvgHandle]: struct.SvgHandle.html
#[derive(Default)]
pub struct Loader {
    unlimited_size: bool,
    keep_image_data: bool,
}

impl Loader {
    /// Creates a `Loader` with the default flags.
    ///
    /// * [`unlimited_size`](#method.with_unlimited_size) defaults to `false`, as malicious
    /// SVG documents could cause the XML parser to consume very large amounts of memory.
    ///
    /// * [`keep_image_data`](#method.keep_image_data) defaults to
    /// `false`.  You may only need this if rendering to Cairo
    /// surfaces that support including image data in compressed
    /// formats, like PDF.
    ///
    /// # Example:
    ///
    /// ```
    /// use librsvg;
    ///
    /// let svg_handle = librsvg::Loader::new()
    ///     .read_path("example.svg")
    ///     .unwrap();
    /// ```
    pub fn new() -> Self {
        Self::default()
    }

    /// Controls safety limits used in the XML parser.
    ///
    /// Internally, librsvg uses libxml2, which has set limits for things like the
    /// maximum length of XML element names, the size of accumulated buffers
    /// using during parsing of deeply-nested XML files, and the maximum size
    /// of embedded XML entities.
    ///
    /// Set this to `true` only if loading a trusted SVG fails due to size limits.
    ///
    /// # Example:
    /// ```
    /// use librsvg;
    ///
    /// let svg_handle = librsvg::Loader::new()
    ///     .with_unlimited_size()
    ///     .read_path("example.svg")    // presumably a trusted huge file
    ///     .unwrap();
    /// ```
    pub fn with_unlimited_size(mut self) -> Self {
        self.unlimited_size = true;
        self
    }

    /// Controls embedding of compressed image data into the renderer.
    ///
    /// Normally, Cairo expects one to pass it uncompressed (decoded)
    /// images as surfaces.  However, when using a PDF rendering
    /// context to render SVG documents that reference raster images
    /// (e.g. those which include a bitmap as part of the SVG image),
    /// it may be more efficient to embed the original, compressed raster
    /// images into the PDF.
    ///
    /// Set this to `true` if you are using a Cairo PDF context, or any other type
    /// of context which allows embedding compressed images.
    ///
    /// # Example:
    ///
    /// ```ignore
    /// # // Test is ignored because "make distcheck" breaks, as the output file
    /// # // can't be written to the read-only srcdir.
    ///
    /// let svg_handle = librsvg::Loader::new()
    ///     .keep_image_data()
    ///     .read_path("example.svg")
    ///     .unwrap();
    ///
    /// let surface = cairo::PdfSurface::new(640.0, 480.0, "output.pdf");
    /// let cr = cairo::Context::new(&surface);
    ///
    /// let renderer = librsvg::CairoRenderer::new(&svg_handle);
    /// renderer.render_document(
    ///     &cr,
    ///     &cairo::Rectangle { x: 0.0, y: 0.0, width: 640.0, height: 480.0 },
    /// ).unwrap();
    /// ```
    pub fn keep_image_data(mut self) -> Self {
        self.keep_image_data = true;
        self
    }

    /// Reads an SVG document from `path`.
    ///
    /// # Example:
    ///
    /// ```
    /// let svg_handle = librsvg::Loader::new()
    ///     .read_path("example.svg")
    ///     .unwrap();
    /// ```
    pub fn read_path<P: AsRef<Path>>(self, path: P) -> Result<SvgHandle, LoadingError> {
        let file = gio::File::new_for_path(path);
        self.read_file(&file, None::<&Cancellable>)
    }

    /// Reads an SVG document from a `gio::File`.
    ///
    /// The `cancellable` can be used to cancel loading from another thread.
    ///
    /// # Example:
    /// ```
    /// let svg_handle = librsvg::Loader::new()
    ///     .read_file(&gio::File::new_for_path("example.svg"), None::<&gio::Cancellable>)
    ///     .unwrap();
    /// ```
    pub fn read_file<F: IsA<gio::File>, P: IsA<Cancellable>>(
        self,
        file: &F,
        cancellable: Option<&P>,
    ) -> Result<SvgHandle, LoadingError> {
        let stream = file.read(cancellable)?;
        self.read_stream(&stream, Some(file), cancellable)
    }

    /// Reads an SVG stream from a `gio::InputStream`.
    ///
    /// This is similar to the [`read`](#method.read) method, but
    /// takes a `gio::InputStream`.  The `base_file`, if it is not
    /// `None`, is used to extract the base URL for this stream.
    ///
    /// Reading an SVG document may involve resolving relative URLs if the
    /// SVG references things like raster images, or other SVG files.
    /// In this case, pass the `base_file` that correspondds to the
    /// URL where this SVG got loaded from.
    ///
    /// The `cancellable` can be used to cancel loading from another thread.
    ///
    /// # Example
    ///
    /// ```
    /// use gio::prelude::*;
    ///
    /// let file = gio::File::new_for_path("example.svg");
    ///
    /// let stream = file.read(None::<&gio::Cancellable>).unwrap();
    ///
    /// let svg_handle = librsvg::Loader::new()
    ///     .read_stream(&stream, Some(&file), None::<&gio::Cancellable>)
    ///     .unwrap();
    /// ```
    pub fn read_stream<S: IsA<gio::InputStream>, F: IsA<gio::File>, P: IsA<Cancellable>>(
        self,
        stream: &S,
        base_file: Option<&F>,
        cancellable: Option<&P>,
    ) -> Result<SvgHandle, LoadingError> {
        let base_file = base_file.map(|f| f.as_ref());

        let base_url = if let Some(base_file) = base_file {
            Some(url_from_file(base_file)?)
        } else {
            None
        };

        let load_options = LoadOptions::new(base_url)
            .with_unlimited_size(self.unlimited_size)
            .keep_image_data(self.keep_image_data);

        Ok(SvgHandle(Handle::from_stream(
            &load_options,
            stream.as_ref(),
            cancellable.map(|c| c.as_ref()),
        )?))
    }
}

fn url_from_file(file: &gio::File) -> Result<Url, LoadingError> {
    Ok(Url::parse(&file.get_uri()).map_err(|_| LoadingError::BadUrl)?)
}

/// Handle used to hold SVG data in memory.
///
/// You can create this from one of the `read` methods in
/// [`Loader`](#struct.Loader.html).
pub struct SvgHandle(Handle);

impl SvgHandle {
    /// Checks if the SVG has an element with the specified `id`.
    ///
    /// Note that the `id` must be a plain fragment identifier like `#foo`, with
    /// a leading `#` character.
    ///
    /// The purpose of the `Err()` case in the return value is to indicate an
    /// incorrectly-formatted `id` argument.
    pub fn has_element_with_id(&self, id: &str) -> Result<bool, RenderingError> {
        self.0.has_sub(id)
    }

    /// Sets a CSS stylesheet to use for an SVG document.
    ///
    /// During the CSS cascade, the specified stylesheet will be used
    /// with a "User" [origin].
    ///
    /// Note that `@import` rules will not be resolved, except for `data:` URLs.
    ///
    /// [origin]: https://drafts.csswg.org/css-cascade-3/#cascading-origins
    pub fn set_stylesheet(&mut self, css: &str) -> Result<(), LoadingError> {
        self.0.set_stylesheet(css)
    }
}

/// Can render an `SvgHandle` to a Cairo context.
pub struct CairoRenderer<'a> {
    handle: &'a SvgHandle,
    dpi: Dpi,
}

// Note that these are different than the C API's default, which is 90.
const DEFAULT_DPI_X: f64 = 96.0;
const DEFAULT_DPI_Y: f64 = 96.0;

#[derive(Debug, Copy, Clone, PartialEq)]
/// Contains the values of the `<svg>` element's `width`, `height`, and `viewBox` attributes.
///
/// An SVG document has a toplevel `<svg>` element, with optional attributes `width`,
/// `height`, and `viewBox`.  This structure contains the values for those attributes; you
/// can obtain the struct from [`CairoRenderer::intrinsic_dimensions`].
///
/// As an example, the following SVG element has a `width` of 100 pixels
/// and a `height` of 400 pixels, but no `viewBox`.
///
/// ```xml
/// <svg xmlns="http://www.w3.org/2000/svg" width="100" height="400">
/// ```
/// In this case, the length fields will be set to `Some()`, and `vbox` to `None`.
///
/// [`CairoRenderer::intrinsic_dimensions`]: struct.CairoRenderer.html#method.intrinsic_dimensions
pub struct IntrinsicDimensions {
    /// `width` attribute of the `<svg>`, if present
    pub width: Option<Length>,

    /// `height` attribute of the `<svg>`, if present
    pub height: Option<Length>,

    /// `viewBox` attribute of the `<svg>`, if present
    pub vbox: Option<cairo::Rectangle>,
}

impl<'a> CairoRenderer<'a> {
    /// Creates a `CairoRenderer` for the specified `SvgHandle`.
    ///
    /// The default dots-per-inch (DPI) value is set to 96; you can change it
    /// with the [`with_dpi`] method.
    ///
    /// [`with_dpi`]: #method.with_dpi
    pub fn new(handle: &'a SvgHandle) -> Self {
        CairoRenderer {
            handle,
            dpi: Dpi::new(DEFAULT_DPI_X, DEFAULT_DPI_Y),
        }
    }

    /// Configures the dots-per-inch for resolving physical lengths.
    ///
    /// If an SVG document has physical units like `5cm`, they must be resolved
    /// to pixel-based values.  The default pixel density is 96 DPI in
    /// both dimensions.
    pub fn with_dpi(self, dpi_x: f64, dpi_y: f64) -> Self {
        assert!(dpi_x > 0.0);
        assert!(dpi_y > 0.0);

        CairoRenderer {
            handle: self.handle,
            dpi: Dpi::new(dpi_x, dpi_y),
        }
    }

    /// Queries the `width`, `height`, and `viewBox` attributes in an SVG document.
    ///
    /// If you are calling this function to compute a scaling factor to render the SVG,
    /// consider simply using [`render_document`] instead; it will do the scaling
    /// computations automatically.
    ///
    /// [`render_document`]: #method.render_document
    pub fn intrinsic_dimensions(&self) -> IntrinsicDimensions {
        let d = self.handle.0.get_intrinsic_dimensions();

        IntrinsicDimensions {
            width: d.width.map(Into::into),
            height: d.height.map(Into::into),
            vbox: d.vbox.map(|v| cairo::Rectangle::from(*v)),
        }
    }

    /// Renders the whole SVG document fitted to a viewport
    ///
    /// The `viewport` gives the position and size at which the whole SVG
    /// document will be rendered.
    ///
    /// The `cr` must be in a `cairo::Status::Success` state, or this function
    /// will not render anything, and instead will return
    /// `RenderingError::Cairo` with the `cr`'s current error state.
    pub fn render_document(
        &self,
        cr: &cairo::Context,
        viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        self.handle.0.render_document(cr, viewport, self.dpi, false)
    }

    /// Computes the (ink_rect, logical_rect) of an SVG element, as if
    /// the SVG were rendered to a specific viewport.
    ///
    /// Element IDs should look like an URL fragment identifier; for
    /// example, pass `Some("#foo")` to get the geometry of the
    /// element that has an `id="foo"` attribute.
    ///
    /// The "ink rectangle" is the bounding box that would be painted
    /// for fully- stroked and filled elements.
    ///
    /// The "logical rectangle" just takes into account the unstroked
    /// paths and text outlines.
    ///
    /// Note that these bounds are not minimum bounds; for example,
    /// clipping paths are not taken into account.
    ///
    /// You can pass `None` for the `id` if you want to measure all
    /// the elements in the SVG, i.e. to measure everything from the
    /// root element.
    ///
    /// This operation is not constant-time, as it involves going through all
    /// the child elements.
    ///
    /// FIXME: example
    pub fn geometry_for_layer(
        &self,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), RenderingError> {
        self.handle
            .0
            .get_geometry_for_layer(id, viewport, self.dpi, false)
            .map(|(i, l)| (i, l))
    }

    /// Renders a single SVG element in the same place as for a whole SVG document
    ///
    /// This is equivalent to `render_document`, but renders only a single element and its
    /// children, as if they composed an individual layer in the SVG.  The element is
    /// rendered with the same transformation matrix as it has within the whole SVG
    /// document.  Applications can use this to re-render a single element and repaint it
    /// on top of a previously-rendered document, for example.
    ///
    /// Note that the `id` must be a plain fragment identifier like `#foo`, with
    /// a leading `#` character.
    ///
    /// The `viewport` gives the position and size at which the whole SVG
    /// document would be rendered.  This function will effectively place the
    /// whole SVG within that viewport, but only render the element given by
    /// `id`.
    ///
    /// The `cr` must be in a `cairo::Status::Success` state, or this function
    /// will not render anything, and instead will return
    /// `RenderingError::Cairo` with the `cr`'s current error state.
    pub fn render_layer(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        self.handle
            .0
            .render_layer(cr, id, viewport, self.dpi, false)
    }

    /// Computes the (ink_rect, logical_rect) of a single SVG element
    ///
    /// While `geometry_for_layer` computes the geometry of an SVG element subtree with
    /// its transformation matrix, this other function will compute the element's geometry
    /// as if it were being rendered under an identity transformation by itself.  That is,
    /// the resulting geometry is as if the element got extracted by itself from the SVG.
    ///
    /// This function is the counterpart to `render_element`.
    ///
    /// Element IDs should look like an URL fragment identifier; for
    /// example, pass `Some("#foo")` to get the geometry of the
    /// element that has an `id="foo"` attribute.
    ///
    /// The "ink rectangle" is the bounding box that would be painted
    /// for fully- stroked and filled elements.
    ///
    /// The "logical rectangle" just takes into account the unstroked
    /// paths and text outlines.
    ///
    /// Note that these bounds are not minimum bounds; for example,
    /// clipping paths are not taken into account.
    ///
    /// You can pass `None` for the `id` if you want to measure all
    /// the elements in the SVG, i.e. to measure everything from the
    /// root element.
    ///
    /// This operation is not constant-time, as it involves going through all
    /// the child elements.
    ///
    /// FIXME: example
    pub fn geometry_for_element(
        &self,
        id: Option<&str>,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), RenderingError> {
        self.handle
            .0
            .get_geometry_for_element(id, self.dpi, false)
            .map(|(i, l)| (i, l))
    }

    /// Renders a single SVG element to a given viewport
    ///
    /// This function can be used to extract individual element subtrees and render them,
    /// scaled to a given `element_viewport`.  This is useful for applications which have
    /// reusable objects in an SVG and want to render them individually; for example, an
    /// SVG full of icons that are meant to be be rendered independently of each other.
    ///
    /// Note that the `id` must be a plain fragment identifier like `#foo`, with
    /// a leading `#` character.
    ///
    /// The `element_viewport` gives the position and size at which the named element will
    /// be rendered.  FIXME: mention proportional scaling.
    ///
    /// The `cr` must be in a `cairo::Status::Success` state, or this function
    /// will not render anything, and instead will return
    /// `RenderingError::Cairo` with the `cr`'s current error state.
    pub fn render_element(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        element_viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        self.handle
            .0
            .render_element(cr, id, element_viewport, self.dpi, false)
    }
}
