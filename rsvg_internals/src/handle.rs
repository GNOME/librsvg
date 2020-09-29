//! Toplevel handle for a loaded SVG document.
//!
//! This module provides the primitives on which the public APIs are implemented.

use crate::allowed_url::{AllowedUrl, Href};
use crate::bbox::BoundingBox;
use crate::css::{Origin, Stylesheet};
use crate::document::{AcquiredNodes, Document};
use crate::dpi::Dpi;
use crate::drawing_ctx::{draw_tree, DrawingMode, ViewParams};
use crate::error::{DefsLookupErrorKind, LoadingError, RenderingError};
use crate::length::*;
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::parsers::Parse;
use crate::rect::Rect;
use crate::structure::IntrinsicDimensions;
use url::Url;

/// Loading options for SVG documents.
#[derive(Clone)]
pub struct LoadOptions {
    /// Base URL; all relative references will be resolved with respect to this.
    pub base_url: Option<Url>,

    /// Whether to turn off size limits in libxml2.
    pub unlimited_size: bool,

    /// Whether to keep original (undecoded) image data to embed in Cairo PDF surfaces.
    pub keep_image_data: bool,
}

impl LoadOptions {
    /// Creates a `LoadOptions` with defaults, and sets the `base_url`.
    pub fn new(base_url: Option<Url>) -> Self {
        LoadOptions {
            base_url,
            unlimited_size: false,
            keep_image_data: false,
        }
    }

    /// Sets whether libxml2's limits on memory usage should be turned off.
    ///
    /// This should only be done for trusted data.
    pub fn with_unlimited_size(mut self, unlimited: bool) -> Self {
        self.unlimited_size = unlimited;
        self
    }

    /// Sets whether to keep the original compressed image data from referenced JPEG/PNG images.
    ///
    /// This is only useful for rendering to Cairo PDF
    /// surfaces, which can embed the original, compressed image data instead of uncompressed
    /// RGB buffers.
    pub fn keep_image_data(mut self, keep: bool) -> Self {
        self.keep_image_data = keep;
        self
    }

    /// Creates a new `LoadOptions` with a different `base_url`.
    ///
    /// This is used when loading a referenced file that may in turn cause other files
    /// to be loaded, for example `<image xlink:href="subimage.svg"/>`
    pub fn copy_with_base_url(&self, base_url: &AllowedUrl) -> Self {
        LoadOptions {
            base_url: Some((**base_url).clone()),
            unlimited_size: self.unlimited_size,
            keep_image_data: self.keep_image_data,
        }
    }
}

/// Main handle to an SVG document.
///
/// This is the main object in librsvg.  It gets created with the [`from_stream`] method
/// and then provides access to all the primitives needed to implement the public APIs.
///
/// [`from_stream`]: #method.from_stream
pub struct Handle {
    document: Document,
}

impl Handle {
    /// Loads an SVG document into a `Handle`.
    pub fn from_stream(
        load_options: &LoadOptions,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Handle, LoadingError> {
        Ok(Handle {
            document: Document::load_from_stream(load_options, stream, cancellable)?,
        })
    }

    /// Queries whether a document has a certain element `#foo`.
    ///
    /// The `id` must be an URL fragment identifier, i.e. something
    /// like `#element_id`.
    pub fn has_sub(&self, id: &str) -> Result<bool, RenderingError> {
        match self.lookup_node(id) {
            Ok(_) => Ok(true),

            Err(DefsLookupErrorKind::NotFound) => Ok(false),

            Err(e) => Err(RenderingError::InvalidId(e)),
        }
    }

    /// Returns (ink_rect, logical_rect)
    pub fn get_geometry_sub(
        &self,
        id: Option<&str>,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(Rect, Rect), RenderingError> {
        let node = self.get_node_or_root(id)?;
        let root = self.document.root();
        let is_root = node == root;

        if is_root {
            let cascaded = CascadedValues::new_from_node(&node);

            if let Some((w, h)) = get_svg_size(&self.get_intrinsic_dimensions(), &cascaded, dpi) {
                let rect = Rect::from_size(w, h);
                return Ok((rect, rect));
            }
        }

        let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target);

        let bbox = draw_tree(
            DrawingMode::LimitToStack { node, root },
            &cr,
            unit_rectangle(),
            dpi,
            true,
            is_testing,
            &mut AcquiredNodes::new(&self.document),
        )?;

        let ink_rect = bbox.ink_rect.unwrap_or_default();
        let logical_rect = bbox.rect.unwrap_or_default();

        Ok((ink_rect, logical_rect))
    }

    fn get_node_or_root(&self, id: Option<&str>) -> Result<Node, RenderingError> {
        if let Some(id) = id {
            self.lookup_node(id).map_err(RenderingError::InvalidId)
        } else {
            Ok(self.document.root())
        }
    }

    pub fn get_geometry_for_layer(
        &self,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), RenderingError> {
        let node = self.get_node_or_root(id)?;
        let root = self.document.root();

        let viewport = Rect::from(*viewport);

        let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target);

        let bbox = draw_tree(
            DrawingMode::LimitToStack { node, root },
            &cr,
            viewport,
            dpi,
            true,
            is_testing,
            &mut AcquiredNodes::new(&self.document),
        )?;

        let ink_rect = bbox.ink_rect.unwrap_or_default();
        let logical_rect = bbox.rect.unwrap_or_default();

        Ok((
            cairo::Rectangle::from(ink_rect),
            cairo::Rectangle::from(logical_rect),
        ))
    }

    fn lookup_node(&self, id: &str) -> Result<Node, DefsLookupErrorKind> {
        match Href::parse(&id).map_err(DefsLookupErrorKind::HrefError)? {
            Href::PlainUrl(_) => Err(DefsLookupErrorKind::CannotLookupExternalReferences),
            Href::WithFragment(fragment) => {
                if let Some(uri) = fragment.uri() {
                    // The public APIs to get geometries of individual elements, or to render
                    // them, should only allow referencing elements within the main handle's
                    // SVG file; that is, only plain "#foo" fragment IDs are allowed here.
                    // Otherwise, a calling program could request "another-file#foo" and cause
                    // another-file to be loaded, even if it is not part of the set of
                    // resources that the main SVG actually references.  In the future we may
                    // relax this requirement to allow lookups within that set, but not to
                    // other random files.

                    let msg = format!(
                        "the public API is not allowed to look up external references: {}#{}",
                        uri,
                        fragment.fragment()
                    );

                    rsvg_log!("{}", msg);

                    return Err(DefsLookupErrorKind::CannotLookupExternalReferences);
                }

                match self.document.lookup_node_by_id(fragment.fragment()) {
                    Some(n) => Ok(n),
                    None => Err(DefsLookupErrorKind::NotFound),
                }
            }
        }
    }

    pub fn render_document(
        &self,
        cr: &cairo::Context,
        viewport: &cairo::Rectangle,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(), RenderingError> {
        self.render_layer(cr, None, viewport, dpi, is_testing)
    }

    pub fn render_layer(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        cr.save();

        let node = self.get_node_or_root(id)?;
        let root = self.document.root();

        let viewport = Rect::from(*viewport);

        let res = draw_tree(
            DrawingMode::LimitToStack { node, root },
            cr,
            viewport,
            dpi,
            false,
            is_testing,
            &mut AcquiredNodes::new(&self.document),
        );

        cr.restore();

        res.map(|_bbox| ())
    }

    fn get_bbox_for_element(
        &self,
        node: &Node,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target);

        let node = node.clone();

        draw_tree(
            DrawingMode::OnlyNode(node),
            &cr,
            unit_rectangle(),
            dpi,
            true,
            is_testing,
            &mut AcquiredNodes::new(&self.document),
        )
    }

    /// Returns (ink_rect, logical_rect)
    pub fn get_geometry_for_element(
        &self,
        id: Option<&str>,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), RenderingError> {
        let node = self.get_node_or_root(id)?;

        let bbox = self.get_bbox_for_element(&node, dpi, is_testing)?;

        let ink_rect = bbox.ink_rect.unwrap_or_default();
        let logical_rect = bbox.rect.unwrap_or_default();

        // Translate so ink_rect is always at offset (0, 0)
        let ofs = (-ink_rect.x0, -ink_rect.y0);

        Ok((
            cairo::Rectangle::from(ink_rect.translate(ofs)),
            cairo::Rectangle::from(logical_rect.translate(ofs)),
        ))
    }

    pub fn render_element(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        element_viewport: &cairo::Rectangle,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        let node = self.get_node_or_root(id)?;

        let bbox = self.get_bbox_for_element(&node, dpi, is_testing)?;

        if bbox.ink_rect.is_none() || bbox.rect.is_none() {
            // Nothing to draw
            return Ok(());
        }

        let ink_r = bbox.ink_rect.unwrap_or_default();

        if ink_r.is_empty() {
            return Ok(());
        }

        // Render, transforming so element is at the new viewport's origin

        cr.save();

        let factor =
            (element_viewport.width / ink_r.width()).min(element_viewport.height / ink_r.height());

        cr.translate(element_viewport.x, element_viewport.y);
        cr.scale(factor, factor);
        cr.translate(-ink_r.x0, -ink_r.y0);

        let res = draw_tree(
            DrawingMode::OnlyNode(node),
            &cr,
            unit_rectangle(),
            dpi,
            false,
            is_testing,
            &mut AcquiredNodes::new(&self.document),
        );

        cr.restore();

        res.map(|_bbox| ())
    }

    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        borrow_element_as!(self.document.root(), Svg).get_intrinsic_dimensions()
    }

    pub fn set_stylesheet(&mut self, css: &str) -> Result<(), LoadingError> {
        let mut stylesheet = Stylesheet::new(Origin::User);
        stylesheet.parse(css, None)?;
        self.document.cascade(&[stylesheet]);
        Ok(())
    }
}

fn check_cairo_context(cr: &cairo::Context) -> Result<(), RenderingError> {
    let status = cr.status();
    if status == cairo::Status::Success {
        Ok(())
    } else {
        Err(RenderingError::Cairo(status))
    }
}

fn unit_rectangle() -> Rect {
    Rect::from_size(1.0, 1.0)
}

/// Returns the SVG's size suitable for the legacy C API, or None
/// if it must be computed by hand.
///
/// The legacy C API can compute an SVG document's size from the
/// `width`, `height`, and `viewBox` attributes of the toplevel `<svg>`
/// element.  If these are not available, then the size must be computed
/// by actually measuring the geometries of elements in the document.
fn get_svg_size(
    dimensions: &IntrinsicDimensions,
    cascaded: &CascadedValues,
    dpi: Dpi,
) -> Option<(f64, f64)> {
    let values = cascaded.get();

    // these defaults are per the spec
    let w = dimensions
        .width
        .unwrap_or_else(|| Length::<Horizontal>::parse_str("100%").unwrap());
    let h = dimensions
        .height
        .unwrap_or_else(|| Length::<Vertical>::parse_str("100%").unwrap());

    match (w, h, dimensions.vbox) {
        (w, h, Some(vbox)) => {
            let params = ViewParams::new(dpi, vbox.width(), vbox.height());

            Some((w.normalize(values, &params), h.normalize(values, &params)))
        }

        (w, h, None) if w.unit != LengthUnit::Percent && h.unit != LengthUnit::Percent => {
            let params = ViewParams::new(dpi, 0.0, 0.0);

            Some((w.normalize(values, &params), h.normalize(values, &params)))
        }
        (_, _, _) => None,
    }
}
