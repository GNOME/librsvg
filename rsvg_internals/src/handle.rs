use std::cell::Cell;
use std::ptr;
use std::rc::Rc;

use cairo::{self, ImageSurface, Status};
use gio;
use glib;
use libc;
use locale_config::{LanguageRange, Locale};

use crate::allowed_url::{AllowedUrl, Href};
use crate::bbox::BoundingBox;
use crate::dpi::Dpi;
use crate::drawing_ctx::{DrawingCtx, RsvgRectangle};
use crate::error::{DefsLookupErrorKind, LoadingError, RenderingError};
use crate::node::{CascadedValues, RsvgNode};
use crate::structure::{IntrinsicDimensions, NodeSvg};
use crate::svg::Svg;
use url::Url;

#[derive(Clone)]
pub struct LoadOptions {
    /// Base URL
    pub base_url: Option<Url>,

    /// Whether to turn off size limits in libxml2
    pub unlimited_size: bool,

    /// Whether to keep original (undecoded) image data to embed in Cairo PDF surfaces
    pub keep_image_data: bool,

    locale: Locale,
}

impl LoadOptions {
    pub fn new(base_url: Option<Url>) -> Self {
        LoadOptions {
            base_url,
            unlimited_size: false,
            keep_image_data: false,
            locale: locale_from_environment(),
        }
    }

    pub fn with_unlimited_size(mut self, unlimited: bool) -> Self {
        self.unlimited_size = unlimited;
        self
    }

    pub fn keep_image_data(mut self, keep: bool) -> Self {
        self.keep_image_data = keep;
        self
    }

    pub fn copy_with_base_url(&self, base_url: &AllowedUrl) -> Self {
        LoadOptions {
            base_url: Some((*base_url).clone()),
            unlimited_size: self.unlimited_size,
            keep_image_data: self.keep_image_data,
            locale: self.locale.clone(),
        }
    }

    pub fn locale(&self) -> &Locale {
        &self.locale
    }
}

// Keep in sync with rsvg.h:RsvgDimensionData
#[repr(C)]
pub struct RsvgDimensionData {
    pub width: libc::c_int,
    pub height: libc::c_int,
    pub em: f64,
    pub ex: f64,
}

impl RsvgDimensionData {
    // This is not #[derive(Default)] to make it clear that it
    // shouldn't be the default value for anything; it is actually a
    // special case we use to indicate an error to the public API.
    pub fn empty() -> RsvgDimensionData {
        RsvgDimensionData {
            width: 0,
            height: 0,
            em: 0.0,
            ex: 0.0,
        }
    }
}

// Keep in sync with rsvg.h:RsvgPositionData
#[repr(C)]
pub struct RsvgPositionData {
    pub x: libc::c_int,
    pub y: libc::c_int,
}

// Keep in sync with rsvg.h:RsvgSizeFunc
pub type RsvgSizeFunc = Option<
    unsafe extern "C" fn(
        inout_width: *mut libc::c_int,
        inout_height: *mut libc::c_int,
        user_data: glib_sys::gpointer,
    ),
>;

pub struct SizeCallback {
    pub size_func: RsvgSizeFunc,
    pub user_data: glib_sys::gpointer,
    pub destroy_notify: glib_sys::GDestroyNotify,
    pub in_loop: Cell<bool>,
}

impl SizeCallback {
    pub fn new(
        size_func: RsvgSizeFunc,
        user_data: glib_sys::gpointer,
        destroy_notify: glib_sys::GDestroyNotify,
    ) -> Self {
        SizeCallback {
            size_func,
            user_data,
            destroy_notify,
            in_loop: Cell::new(false),
        }
    }

    pub fn call(&self, width: libc::c_int, height: libc::c_int) -> (libc::c_int, libc::c_int) {
        unsafe {
            let mut w = width;
            let mut h = height;

            if let Some(ref f) = self.size_func {
                f(&mut w, &mut h, self.user_data);
            };

            (w, h)
        }
    }

    pub fn start_loop(&self) {
        assert!(!self.in_loop.get());
        self.in_loop.set(true);
    }

    pub fn end_loop(&self) {
        assert!(self.in_loop.get());
        self.in_loop.set(false);
    }

    pub fn get_in_loop(&self) -> bool {
        self.in_loop.get()
    }
}

impl Default for SizeCallback {
    fn default() -> SizeCallback {
        SizeCallback {
            size_func: None,
            user_data: ptr::null_mut(),
            destroy_notify: None,
            in_loop: Cell::new(false),
        }
    }
}

impl Drop for SizeCallback {
    fn drop(&mut self) {
        unsafe {
            if let Some(ref f) = self.destroy_notify {
                f(self.user_data);
            };
        }
    }
}

pub struct Handle {
    svg: Rc<Svg>,
}

impl Handle {
    pub fn from_stream(
        load_options: &LoadOptions,
        stream: &gio::InputStream,
        cancellable: Option<&gio::Cancellable>,
    ) -> Result<Handle, LoadingError> {
        Ok(Handle {
            svg: Rc::new(Svg::load_from_stream(load_options, stream, cancellable)?),
        })
    }

    pub fn has_sub(&self, id: &str) -> Result<bool, RenderingError> {
        match self.lookup_node(id) {
            Ok(_) => Ok(true),

            Err(DefsLookupErrorKind::NotFound) => Ok(false),

            Err(e) => Err(RenderingError::InvalidId(e)),
        }
    }

    pub fn get_dimensions(
        &self,
        dpi: Dpi,
        size_callback: &SizeCallback,
        is_testing: bool,
    ) -> Result<RsvgDimensionData, RenderingError> {
        // This function is probably called from the cairo_render functions,
        // or is being erroneously called within the size_func.
        // To prevent an infinite loop we are saving the state, and
        // returning a meaningless size.
        if size_callback.get_in_loop() {
            return Ok(RsvgDimensionData {
                width: 1,
                height: 1,
                em: 1.0,
                ex: 1.0,
            });
        }

        size_callback.start_loop();

        let res = self.get_dimensions_sub(None, dpi, size_callback, is_testing);

        size_callback.end_loop();

        res
    }

    pub fn get_dimensions_sub(
        &self,
        id: Option<&str>,
        dpi: Dpi,
        size_callback: &SizeCallback,
        is_testing: bool,
    ) -> Result<RsvgDimensionData, RenderingError> {
        let (ink_r, _) = self.get_geometry_sub(id, dpi, is_testing)?;

        let (w, h) = size_callback.call(ink_r.width as libc::c_int, ink_r.height as libc::c_int);

        Ok(RsvgDimensionData {
            width: w,
            height: h,
            em: ink_r.width,
            ex: ink_r.height,
        })
    }

    pub fn get_position_sub(
        &self,
        id: Option<&str>,
        dpi: Dpi,
        size_callback: &SizeCallback,
        is_testing: bool,
    ) -> Result<RsvgPositionData, RenderingError> {
        if id.is_none() {
            return Ok(RsvgPositionData { x: 0, y: 0 });
        }

        let (ink_r, _) = self.get_geometry_sub(id, dpi, is_testing)?;

        let width = ink_r.width as libc::c_int;
        let height = ink_r.height as libc::c_int;

        size_callback.call(width, height);

        Ok(RsvgPositionData {
            x: ink_r.x as libc::c_int,
            y: ink_r.y as libc::c_int,
        })
    }

    /// Returns (ink_rect, logical_rect)
    fn get_node_geometry_with_viewport(
        &self,
        node: &RsvgNode,
        viewport: &cairo::Rectangle,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let target = ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target);
        let mut draw_ctx = DrawingCtx::new(
            self.svg.clone(),
            Some(node),
            &cr,
            viewport,
            dpi,
            true,
            is_testing,
        );
        let root = self.svg.root();

        let bbox = draw_ctx.draw_node_from_stack(&CascadedValues::new_from_node(&root), &root, false)?;

        let ink_rect = bbox
            .ink_rect
            .map(|r| RsvgRectangle::from(r))
            .unwrap_or_default();
        let logical_rect = bbox
            .rect
            .map(|r| RsvgRectangle::from(r))
            .unwrap_or_default();

        Ok((ink_rect, logical_rect))
    }

    /// Returns (ink_rect, logical_rect)
    fn get_geometry_sub(
        &self,
        id: Option<&str>,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let node = self.get_node_or_root(id)?;

        let root = self.svg.root();
        let is_root = node == root;

        if is_root {
            let cascaded = CascadedValues::new_from_node(&node);
            let values = cascaded.get();

            if let Some((root_width, root_height)) =
                node.borrow().get_impl::<NodeSvg>().get_size(&values, dpi)
            {
                let ink_r = RsvgRectangle {
                    x: 0.0,
                    y: 0.0,
                    width: f64::from(root_width),
                    height: f64::from(root_height),
                };

                let logical_r = ink_r;

                return Ok((ink_r, logical_r));
            }
        }

        self.get_node_geometry_with_viewport(&node, &unit_rectangle(), dpi, is_testing)
    }

    fn get_node_or_root(&self, id: Option<&str>) -> Result<RsvgNode, RenderingError> {
        if let Some(id) = id {
            self.lookup_node(id).map_err(RenderingError::InvalidId)
        } else {
            Ok(self.svg.root())
        }
    }

    pub fn get_geometry_for_layer(
        &self,
        id: Option<&str>,
        viewport: &cairo::Rectangle,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let node = self.get_node_or_root(id)?;
        self.get_node_geometry_with_viewport(&node, viewport, dpi, is_testing)
    }

    fn lookup_node(&self, id: &str) -> Result<RsvgNode, DefsLookupErrorKind> {
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

                match self.svg.lookup_node_by_id(fragment.fragment()) {
                    Some(n) => Ok(n),
                    None => Err(DefsLookupErrorKind::NotFound),
                }
            }
        }
    }

    pub fn render_cairo_sub(
        &self,
        cr: &cairo::Context,
        id: Option<&str>,
        dpi: Dpi,
        size_callback: &SizeCallback,
        is_testing: bool,
    ) -> Result<(), RenderingError> {
        check_cairo_context(cr)?;

        let dimensions = self.get_dimensions(dpi, size_callback, is_testing)?;
        if dimensions.width == 0 || dimensions.height == 0 {
            // nothing to render
            return Ok(());
        }

        let viewport = cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width: f64::from(dimensions.width),
            height: f64::from(dimensions.height),
        };

        self.render_layer(cr, id, &viewport, dpi, is_testing)
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

        let node = if let Some(id) = id {
            Some(self.lookup_node(id).map_err(RenderingError::InvalidId)?)
        } else {
            None
        };

        let root = self.svg.root();

        cr.save();
        let mut draw_ctx = DrawingCtx::new(
            self.svg.clone(),
            node.as_ref(),
            cr,
            viewport,
            dpi,
            false,
            is_testing,
        );
        let cascaded = CascadedValues::new_from_node(&root);
        let res = draw_ctx
            .draw_node_from_stack(&cascaded, &root, false)
            .map(|_bbox| ());
        cr.restore();

        res
    }

    fn get_bbox_for_element(
        &self,
        node: &RsvgNode,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let target = ImageSurface::create(cairo::Format::Rgb24, 1, 1)?;
        let cr = cairo::Context::new(&target);

        let mut draw_ctx = DrawingCtx::new(
            self.svg.clone(),
            None,
            &cr,
            &unit_rectangle(),
            dpi,
            true,
            is_testing,
        );

        draw_ctx.draw_node_from_stack(&CascadedValues::new_from_node(node), node, false)
    }

    /// Returns (ink_rect, logical_rect)
    pub fn get_geometry_for_element(
        &self,
        id: Option<&str>,
        dpi: Dpi,
        is_testing: bool,
    ) -> Result<(RsvgRectangle, RsvgRectangle), RenderingError> {
        let node = self.get_node_or_root(id)?;

        let bbox = self.get_bbox_for_element(&node, dpi, is_testing)?;

        let mut ink_rect = bbox
            .ink_rect
            .map(|r| RsvgRectangle::from(r))
            .unwrap_or_default();
        let mut logical_rect = bbox
            .rect
            .map(|r| RsvgRectangle::from(r))
            .unwrap_or_default();

        // Translate so ink_rect is always at offset (0, 0)

        let xofs = ink_rect.x;
        let yofs = ink_rect.y;

        ink_rect.x -= xofs;
        ink_rect.y -= yofs;

        logical_rect.x -= xofs;
        logical_rect.y -= yofs;

        Ok((ink_rect, logical_rect))
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

        let ink_r = bbox
            .ink_rect
            .map(|r| RsvgRectangle::from(r))
            .unwrap_or_default();

        if ink_r.width == 0.0 || ink_r.height == 0.0 {
            return Ok(());
        }

        // Render, transforming so element is at the new viewport's origin

        cr.save();

        let factor =
            (element_viewport.width / ink_r.width).min(element_viewport.height / ink_r.height);

        cr.translate(element_viewport.x, element_viewport.y);
        cr.scale(factor, factor);
        cr.translate(-ink_r.x, -ink_r.y);

        let mut draw_ctx = DrawingCtx::new(
            self.svg.clone(),
            None,
            &cr,
            &unit_rectangle(),
            dpi,
            false,
            is_testing,
        );

        let res = draw_ctx
            .draw_node_from_stack(&CascadedValues::new_from_node(&node), &node, false)
            .map(|_bbox| ());

        cr.restore();

        res
    }

    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        self.svg.get_intrinsic_dimensions()
    }
}

fn check_cairo_context(cr: &cairo::Context) -> Result<(), RenderingError> {
    let status = cr.status();
    if status == Status::Success {
        Ok(())
    } else {
        Err(RenderingError::Cairo(status))
    }
}

/// Gets the user's preferred locale from the environment and
/// translates it to a `Locale` with `LanguageRange` fallbacks.
///
/// The `Locale::current()` call only contemplates a single language,
/// but glib is smarter, and `g_get_langauge_names()` can provide
/// fallbacks, for example, when LC_MESSAGES="en_US.UTF-8:de" (USA
/// English and German).  This function converts the output of
/// `g_get_language_names()` into a `Locale` with appropriate
/// fallbacks.
fn locale_from_environment() -> Locale {
    let mut locale = Locale::invariant();

    for name in glib::get_language_names() {
        if let Ok(range) = LanguageRange::from_unix(&name) {
            locale.add(&range);
        }
    }

    locale
}

fn unit_rectangle() -> cairo::Rectangle {
    cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: 1.0,
        height: 1.0,
    }
}
