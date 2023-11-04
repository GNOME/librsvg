//! Toplevel handle for a loaded SVG document.
//!
//! This module provides the primitives on which the public APIs are implemented.

use std::sync::Arc;

use crate::accept_language::UserLanguage;
use crate::bbox::BoundingBox;
use crate::borrow_element_as;
use crate::css::{Origin, Stylesheet};
use crate::document::{AcquiredNodes, Document};
use crate::dpi::Dpi;
use crate::drawing_ctx::{draw_tree, with_saved_cr, DrawingMode};
use crate::error::{InternalRenderingError, LoadingError};
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::rect::Rect;
use crate::session::Session;
use crate::structure::IntrinsicDimensions;
use crate::url_resolver::{AllowedUrl, UrlResolver};

/// Loading options for SVG documents.
pub struct LoadOptions {
    /// Load url resolver; all references will be resolved with respect to this.
    pub url_resolver: UrlResolver,

    /// Whether to turn off size limits in libxml2.
    pub unlimited_size: bool,

    /// Whether to keep original (undecoded) image data to embed in Cairo PDF surfaces.
    pub keep_image_data: bool,
}

impl LoadOptions {
    /// Creates a `LoadOptions` with defaults, and sets the `url resolver`.
    pub fn new(url_resolver: UrlResolver) -> Self {
        LoadOptions {
            url_resolver,
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

    /// Creates a new `LoadOptions` with a different `url resolver`.
    ///
    /// This is used when loading a referenced file that may in turn cause other files
    /// to be loaded, for example `<image xlink:href="subimage.svg"/>`
    pub fn copy_with_base_url(&self, base_url: &AllowedUrl) -> Self {
        let mut url_resolver = self.url_resolver.clone();
        url_resolver.base_url = Some((**base_url).clone());

        LoadOptions {
            url_resolver,
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
    session: Session,
    pub(crate) document: Document,
}

impl Handle {
    /// Loads an SVG document into a `Handle`.
    pub fn from_stream(
        session: Session,
        load_options: Arc<LoadOptions>,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Handle, LoadingError> {
        Ok(Handle {
            session: session.clone(),
            document: Document::load_from_stream(session, load_options, stream, cancellable)?,
        })
    }

    fn geometry_for_layer(
        &self,
        node: Node,
        viewport: Rect,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(Rect, Rect), InternalRenderingError> {
        let root = self.document.root();

        let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target)?;

        let bbox = draw_tree(
            self.session.clone(),
            DrawingMode::LimitToStack { node, root },
            &cr,
            viewport,
            user_language,
            dpi,
            true,
            is_testing,
            &mut AcquiredNodes::new(&self.document),
        )?;

        let ink_rect = bbox.ink_rect.unwrap_or_default();
        let logical_rect = bbox.rect.unwrap_or_default();

        Ok((ink_rect, logical_rect))
    }

    pub fn get_geometry_for_layer(
        &self,
        node: Node,
        viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), InternalRenderingError> {
        let viewport = Rect::from(*viewport);

        let (ink_rect, logical_rect) =
            self.geometry_for_layer(node, viewport, user_language, dpi, is_testing)?;

        Ok((
            cairo::Rectangle::from(ink_rect),
            cairo::Rectangle::from(logical_rect),
        ))
    }

    pub fn render_document(
        &self,
        cr: &cairo::Context,
        viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(), InternalRenderingError> {
        let root = self.document.root();
        self.render_layer(cr, root, viewport, user_language, dpi, is_testing)
    }

    pub fn render_layer(
        &self,
        cr: &cairo::Context,
        node: Node,
        viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(), InternalRenderingError> {
        cr.status()?;

        let root = self.document.root();

        let viewport = Rect::from(*viewport);

        with_saved_cr(cr, || {
            draw_tree(
                self.session.clone(),
                DrawingMode::LimitToStack { node, root },
                cr,
                viewport,
                user_language,
                dpi,
                false,
                is_testing,
                &mut AcquiredNodes::new(&self.document),
            )
            .map(|_bbox| ())
        })
    }

    fn get_bbox_for_element(
        &self,
        node: &Node,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        let target = cairo::ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target)?;

        let node = node.clone();

        draw_tree(
            self.session.clone(),
            DrawingMode::OnlyNode(node),
            &cr,
            unit_rectangle(),
            user_language,
            dpi,
            true,
            is_testing,
            &mut AcquiredNodes::new(&self.document),
        )
    }

    /// Returns (ink_rect, logical_rect)
    pub fn get_geometry_for_element(
        &self,
        node: Node,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(cairo::Rectangle, cairo::Rectangle), InternalRenderingError> {
        let bbox = self.get_bbox_for_element(&node, user_language, dpi, is_testing)?;

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
        node: Node,
        element_viewport: &cairo::Rectangle,
        user_language: &UserLanguage,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(), InternalRenderingError> {
        cr.status()?;

        let bbox = self.get_bbox_for_element(&node, user_language, dpi, is_testing)?;

        if bbox.ink_rect.is_none() || bbox.rect.is_none() {
            // Nothing to draw
            return Ok(());
        }

        let ink_r = bbox.ink_rect.unwrap_or_default();

        if ink_r.is_empty() {
            return Ok(());
        }

        // Render, transforming so element is at the new viewport's origin

        with_saved_cr(cr, || {
            let factor = (element_viewport.width() / ink_r.width())
                .min(element_viewport.height() / ink_r.height());

            cr.translate(element_viewport.x(), element_viewport.y());
            cr.scale(factor, factor);
            cr.translate(-ink_r.x0, -ink_r.y0);

            draw_tree(
                self.session.clone(),
                DrawingMode::OnlyNode(node),
                cr,
                unit_rectangle(),
                user_language,
                dpi,
                false,
                is_testing,
                &mut AcquiredNodes::new(&self.document),
            )
            .map(|_bbox| ())
        })
    }

    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        let root = self.document.root();
        let cascaded = CascadedValues::new_from_node(&root);
        let values = cascaded.get();
        borrow_element_as!(self.document.root(), Svg).get_intrinsic_dimensions(values)
    }

    pub fn set_stylesheet(&mut self, css: &str) -> Result<(), LoadingError> {
        let stylesheet = Stylesheet::from_data(
            css,
            &UrlResolver::new(None),
            Origin::User,
            self.session.clone(),
        )?;
        self.document.cascade(&[stylesheet], &self.session);
        Ok(())
    }
}

fn unit_rectangle() -> Rect {
    Rect::from_size(1.0, 1.0)
}
