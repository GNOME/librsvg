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
//! ```ignore
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
//! or in `/foo/bar/*/.../*`.  This is so that malicious SVG files
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

#![warn(unused)]
use cairo;
use gio;
use glib;
use rsvg_internals;

use std::path::Path;

use gio::{Cancellable, FileExt};
use glib::object::Cast;

use rsvg_internals::{Dpi, Handle, LoadFlags};

pub use rsvg_internals::{
    DefsLookupErrorKind,
    HrefError,
    Length,
    LengthUnit,
    LoadingError,
    RenderingError,
};

/// Struct for loading an [`SvgHandle`][SvgHandle].
///
/// This is the starting point for using librsvg.  This struct
/// implements a builder pattern for configuring an
/// [`SvgHandle`][SvgHandle]'s options, and then loading the SVG data.
/// You can call the methods of `Loader` in sequence to configure
/// how SVG data should be loaded, and finally use one of the loading
/// functions to load an [`SvgHandle`][SvgHandle].
///
/// [SvgHandle]: struct.SvgHandle.html
pub struct Loader {
    unlimited_size: bool,
    keep_image_data: bool,
}

impl Loader {
    /// Creates a `Loader` with the default flags.
    ///
    /// * [`unlimited_size`](#method.with_unlimited_size) defaults to `false`, as malicious
    /// SVG files could cause the XML parser to consume very large amounts of memory.
    ///
    /// * [`keep_image_data`](#method.keep_image_data) defaults to
    /// `false`.  You may only need this if rendering to Cairo
    /// surfaces that support including image data in compressed
    /// formats, like PDF.
    ///
    /// # Example:
    ///
    /// ```ignore
    /// use librsvg;
    ///
    /// use librsvg::Loader;
    ///
    /// let svg_handle = Loader::new()
    ///     .read_path("example.svg")
    ///     .unwrap();
    /// ```
    pub fn new() -> Self {
        Loader {
            unlimited_size: false,
            keep_image_data: false,
        }
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
    /// ```ignore
    /// use librsvg;
    ///
    /// use librsvg::Loader;
    ///
    /// let svg_handle = Loader::new()
    ///     .with_unlimited_size()
    ///     .read_path("trusted-huge-file.svg")
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
    /// context to render SVG files that reference raster images
    /// (e.g. those which include a bitmap as part of the SVG image),
    /// it may be more efficient to embed the original, compressed raster
    /// images into the PDF.
    ///
    /// Set this to `true` if you are using a Cairo PDF context, or any other type
    /// of context which allows embedding compressed images.
    ///
    /// # Example:
    /// ```ignore
    /// use cairo;
    /// use librsvg;
    ///
    /// use librsvg::Loader;
    ///
    /// let svg_handle = Loader::new()
    ///     .keep_image_data()
    ///     .read_path("svg-with-embedded-images.svg")
    ///     .unwrap();
    ///
    /// let surface = cairo::pdf::File::new(..., "hello.pdf");
    /// let cr = cairo::Context::new(&surface);
    ///
    /// let renderer = CairoRenderer::new(&svg_handle);
    /// renderer.render(&cr).unwrap();
    /// ```
    pub fn keep_image_data(mut self) -> Self {
        self.keep_image_data = true;
        self
    }

    fn load_flags(&self) -> LoadFlags {
        LoadFlags {
            unlimited_size: self.unlimited_size,
            keep_image_data: self.keep_image_data,
        }
    }

    /// Reads an SVG file from `path`.
    ///
    /// # Example:
    /// ```ignore
    /// use librsvg;
    ///
    /// use librsvg::Loader;
    ///
    /// let svg_handle = Loader::new()
    ///     .read_path("hello.svg")
    ///     .unwrap();
    /// ```
    pub fn read_path<P: AsRef<Path>>(self, path: P) -> Result<SvgHandle, LoadingError> {
        let file = gio::File::new_for_path(path);
        self.read_file(&file, None)
    }

    /// Reads an SVG file from a `gio::File`.
    ///
    /// The `cancellable` can be used to cancel loading from another thread.
    ///
    /// # Example:
    /// ```ignore
    /// use gio;
    /// use librsvg;
    ///
    /// use librsvg::Loader;
    ///
    /// let svg_handle = Loader::new()
    ///     .read_file(&gio::File::new_for_path("hello.svg"), None)
    ///     .unwrap();
    /// ```
    pub fn read_file<'a, P: Into<Option<&'a Cancellable>>>(
        self,
        file: &gio::File,
        cancellable: P,
    ) -> Result<SvgHandle, LoadingError> {
        let cancellable = cancellable.into();

        let cancellable_clone = cancellable.clone();

        let stream = file.read(cancellable)?;

        self.read_stream(&stream.upcast(), Some(&file), cancellable_clone)
    }

    /// Reads an SVG stream from a `gio::InputStream`.
    ///
    /// This is similar to the [`read`](#method.read) method, but
    /// takes a `gio::InputStream`.  The `base_file`, if it is not
    /// `None`, is used to extract the base URL for this stream.
    ///
    /// Reading an SVG file may involve resolving relative URLs if the
    /// SVG references things like raster images, or other SVG files.
    /// In this case, pass the `base_file` that correspondds to the
    /// URL where this SVG got loaded from.
    ///
    /// The `cancellable` can be used to cancel loading from another thread.
    pub fn read_stream<'a, P: Into<Option<&'a Cancellable>>>(
        self,
        stream: &gio::InputStream,
        base_file: Option<&gio::File>,
        cancellable: P,
    ) -> Result<SvgHandle, LoadingError> {
        let handle = Handle::new_with_flags(self.load_flags());
        handle.construct_read_stream_sync(stream, base_file, cancellable.into())?;

        Ok(SvgHandle(handle))
    }
}

/// Handle used to hold SVG data in memory.
///
/// You can create this from one of the `read` methods in
/// [`Loader`](#struct.Loader.html).
pub struct SvgHandle(Handle);

/// Can render an `SvgHandle` to a Cairo context.
pub struct CairoRenderer<'a> {
    handle: &'a SvgHandle,
    dpi: Dpi,
}

// Note that these are different than the C API's default, which is 90.
const DEFAULT_DPI_X: f64 = 96.0;
const DEFAULT_DPI_Y: f64 = 96.0;

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct IntrinsicDimensions {
    pub width: Option<Length>,
    pub height: Option<Length>,
    pub vbox: Option<cairo::Rectangle>,
}

impl<'a> CairoRenderer<'a> {
    /// Creates a `CairoRenderer` for the specified `SvgHandle`.
    ///
    /// If an SVG file has physical units like `5cm`, they must be resolved
    /// to pixel-based values.  The default pixel density is `96.0` DPI in
    /// both dimensions.
    pub fn new(handle: &'a SvgHandle) -> Self {
        CairoRenderer {
            handle,
            dpi: Dpi::new(DEFAULT_DPI_X, DEFAULT_DPI_Y),
        }
    }

    /// Configures the dots-per-inch for resolving physical lengths.
    pub fn with_dpi(self, dpi_x: f64, dpi_y: f64) -> Self {
        assert!(dpi_x > 0.0);
        assert!(dpi_y > 0.0);

        CairoRenderer {
            handle: self.handle,
            dpi: Dpi::new(dpi_x, dpi_y),
        }
    }

    #[cfg(test)]
    pub fn dimensions(&self) -> Result<(i32, i32), RenderingError> {
        self.handle
            .0
            .get_dimensions()
            .map(|dimensions| (dimensions.width, dimensions.height))
    }

    #[cfg(test)]
    pub fn render_cairo_sub(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
    ) -> Result<(), RenderingError> {
        self.handle.0.render_cairo_sub(cr, id)
    }

    pub fn intrinsic_dimensions(&self) -> IntrinsicDimensions {
        let d = self.handle.0.get_intrinsic_dimensions();

        IntrinsicDimensions {
            width: d.width.map(|l| l.to_length()),
            height: d.height.map(|l| l.to_length()),
            vbox: d.vbox.map(|v| cairo::Rectangle {
                x: v.x,
                y: v.y,
                width: v.width,
                height: v.height,
            }),
        }
    }

    /// Computes the (ink_rect, logical_rect) of an SVG element, as if
    /// it were rendered to a specific viewport.
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
        viewport: &cairo::Rectangle,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), RenderingError> {
        self.handle
            .0
            .get_geometry_for_element(id, viewport)
            .map(|(i, l)| (i.into(), l.into()))
    }

    pub fn render_element_to_viewport(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
    ) -> Result<(), RenderingError> {
        self.handle.0.render_element_to_viewport(cr, id, viewport)
    }
}
