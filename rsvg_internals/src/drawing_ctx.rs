//! The main context structure which drives the drawing process.

use once_cell::sync::Lazy;
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::{Rc, Weak};

use crate::allowed_url::Fragment;
use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::dasharray::Dasharray;
use crate::document::Document;
use crate::dpi::Dpi;
use crate::error::{AcquireError, RenderingError};
use crate::filters;
use crate::gradient::{LinearGradient, RadialGradient};
use crate::limits;
use crate::marker;
use crate::node::{CascadedValues, NodeDraw, NodeType, RsvgNode};
use crate::paint_server::{PaintServer, PaintSource};
use crate::path_builder::*;
use crate::pattern::Pattern;
use crate::properties::ComputedValues;
use crate::property_defs::{
    ClipRule, FillRule, Opacity, Overflow, ShapeRendering, StrokeDasharray, StrokeLinecap,
    StrokeLinejoin,
};
use crate::rect::Rect;
use crate::shapes::Markers;
use crate::structure::{ClipPath, Mask, Symbol, Use};
use crate::surface_utils::{
    shared_surface::ExclusiveImageSurface, shared_surface::SharedImageSurface,
    shared_surface::SurfaceType,
};
use crate::transform::Transform;
use crate::unit_interval::UnitInterval;
use crate::viewbox::ViewBox;

/// Holds values that are required to normalize `Length` values to a current viewport.
///
/// This struct is created by calling `DrawingCtx::push_view_box()` or
/// `DrawingCtx::get_view_params()`.
///
/// This struct holds the size of the current viewport in the user's coordinate system.  A
/// viewport pushed with `DrawingCtx::push_view_box()` will remain in place until the
/// returned `ViewParams` is dropped; at that point, the `DrawingCtx` will resume using its
/// previous viewport.
pub struct ViewParams {
    pub dpi_x: f64,
    pub dpi_y: f64,
    pub view_box_width: f64,
    pub view_box_height: f64,
    view_box_stack: Option<Weak<RefCell<Vec<ViewBox>>>>,
}

impl ViewParams {
    pub fn new(dpi_x: f64, dpi_y: f64, view_box_width: f64, view_box_height: f64) -> ViewParams {
        ViewParams {
            dpi_x,
            dpi_y,
            view_box_width,
            view_box_height,
            view_box_stack: None,
        }
    }
}

impl Drop for ViewParams {
    fn drop(&mut self) {
        if let Some(ref weak_stack) = self.view_box_stack {
            let stack = weak_stack
                .upgrade()
                .expect("A ViewParams was dropped after its DrawingCtx!?");
            stack.borrow_mut().pop();
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ClipMode {
    ClipToViewport,
    ClipToVbox,
}

pub struct DrawingCtx<'i> {
    document: &'i Document,

    initial_transform: Transform,

    rect: Rect,
    dpi: Dpi,

    cr_stack: Vec<cairo::Context>,
    cr: cairo::Context,

    view_box_stack: Rc<RefCell<Vec<ViewBox>>>,

    drawsub_stack: Vec<RsvgNode>,

    acquired_nodes: AcquiredNodes<'i>,

    measuring: bool,
    testing: bool,
}

impl<'i> DrawingCtx<'i> {
    pub fn new(
        document: &'i Document,
        node: Option<&RsvgNode>,
        cr: &cairo::Context,
        viewport: Rect,
        dpi: Dpi,
        measuring: bool,
        testing: bool,
    ) -> DrawingCtx<'i> {
        let initial_transform = Transform::from(cr.get_matrix());

        // This is more or less a hack to make measuring geometries possible,
        // while the code gets refactored not to need special cases for that.

        let (rect, vbox) = if measuring {
            let unit_rect = Rect::from_size(1.0, 1.0);
            (unit_rect, ViewBox(unit_rect))
        } else {
            // https://www.w3.org/TR/SVG2/coords.html#InitialCoordinateSystem
            //
            // "For the outermost svg element, the SVG user agent must
            // determine an initial viewport coordinate system and an
            // initial user coordinate system such that the two
            // coordinates systems are identical. The origin of both
            // coordinate systems must be at the origin of the SVG
            // viewport."
            //
            // "... the initial viewport coordinate system (and therefore
            // the initial user coordinate system) must have its origin at
            // the top/left of the viewport"
            let vbox = ViewBox(Rect::from_size(viewport.width(), viewport.height()));

            (viewport, vbox)
        };

        let mut view_box_stack = Vec::new();
        view_box_stack.push(vbox);

        let acquired_nodes = AcquiredNodes::new(document);

        let mut draw_ctx = DrawingCtx {
            document,
            initial_transform,
            rect,
            dpi,
            cr_stack: Vec::new(),
            cr: cr.clone(),
            view_box_stack: Rc::new(RefCell::new(view_box_stack)),
            drawsub_stack: Vec::new(),
            acquired_nodes,
            measuring,
            testing,
        };

        if let Some(node) = node {
            for n in node.ancestors() {
                draw_ctx.drawsub_stack.push(n.clone());
            }
        }

        draw_ctx
    }

    pub fn toplevel_viewport(&self) -> Rect {
        self.rect
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring
    }

    pub fn is_testing(&self) -> bool {
        self.testing
    }

    pub fn get_cairo_context(&self) -> cairo::Context {
        self.cr.clone()
    }

    pub fn get_transform(&self) -> Transform {
        Transform::from(self.cr.get_matrix())
    }

    pub fn empty_bbox(&self) -> BoundingBox {
        BoundingBox::new().with_transform(self.get_transform())
    }

    // FIXME: Usage of this function is more less a hack... The caller
    // manually saves and then restore the draw_ctx.cr.
    // It would be better to have an explicit push/pop for the cairo_t, or
    // pushing a temporary surface, or something that does not involve
    // monkeypatching the cr directly.
    pub fn set_cairo_context(&mut self, cr: &cairo::Context) {
        self.cr = cr.clone();
    }

    // Temporary hack while we unify surface/cr/affine creation
    fn push_cairo_context(&mut self, cr: cairo::Context) {
        self.cr_stack.push(self.cr.clone());
        self.cr = cr;
    }

    // Temporary hack while we unify surface/cr/affine creation
    fn pop_cairo_context(&mut self) {
        self.cr = self.cr_stack.pop().unwrap();
    }

    fn size_for_temporary_surface(&self) -> (i32, i32) {
        let (viewport_width, viewport_height) = (self.rect.width(), self.rect.height());

        let (width, height) = self
            .initial_transform
            .transform_distance(viewport_width, viewport_height);

        // We need a size in whole pixels, so use ceil() to ensure the whole viewport fits
        // into the temporary surface.
        (width.ceil() as i32, height.ceil() as i32)
    }

    pub fn create_surface_for_toplevel_viewport(
        &self,
    ) -> Result<cairo::ImageSurface, RenderingError> {
        let (w, h) = self.size_for_temporary_surface();

        Ok(cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?)
    }

    fn create_similar_surface_for_toplevel_viewport(
        &self,
        surface: &cairo::Surface,
    ) -> Result<cairo::Surface, RenderingError> {
        let (w, h) = self.size_for_temporary_surface();

        Ok(cairo::Surface::create_similar(
            surface,
            cairo::Content::ColorAlpha,
            w,
            h,
        )?)
    }

    /// Gets the viewport that was last pushed with `push_view_box()`.
    pub fn get_view_params(&self) -> ViewParams {
        let view_box_stack = self.view_box_stack.borrow();
        let last = view_box_stack.len() - 1;
        let top_rect = &view_box_stack[last].0;

        ViewParams {
            dpi_x: self.dpi.x(),
            dpi_y: self.dpi.y(),
            view_box_width: top_rect.width(),
            view_box_height: top_rect.height(),
            view_box_stack: None,
        }
    }

    /// Pushes a viewport size for normalizing `Length` values.
    ///
    /// You should pass the returned `ViewParams` to all subsequent `Length.normalize()`
    /// calls that correspond to this viewport.
    ///
    /// The viewport will stay in place, and will be the one returned by
    /// `get_view_params()`, until the returned `ViewParams` is dropped.
    pub fn push_view_box(&self, width: f64, height: f64) -> ViewParams {
        self.view_box_stack
            .borrow_mut()
            .push(ViewBox(Rect::from_size(width, height)));

        ViewParams {
            dpi_x: self.dpi.x(),
            dpi_y: self.dpi.y(),
            view_box_width: width,
            view_box_height: height,
            view_box_stack: Some(Rc::downgrade(&self.view_box_stack)),
        }
    }

    pub fn push_new_viewport(
        &self,
        vbox: Option<ViewBox>,
        viewport: Rect,
        preserve_aspect_ratio: AspectRatio,
        clip_mode: Option<ClipMode>,
    ) -> Option<ViewParams> {
        let cr = self.get_cairo_context();

        if let Some(ClipMode::ClipToViewport) = clip_mode {
            cr.rectangle(
                viewport.x0,
                viewport.y0,
                viewport.width(),
                viewport.height(),
            );
            cr.clip();
        }

        preserve_aspect_ratio
            .viewport_to_viewbox_transform(vbox, &viewport)
            .and_then(|t| {
                self.cr.transform(t.into());

                if let Some(vbox) = vbox {
                    if let Some(ClipMode::ClipToVbox) = clip_mode {
                        cr.rectangle(vbox.0.x0, vbox.0.y0, vbox.0.width(), vbox.0.height());
                        cr.clip();
                    }

                    Some(self.push_view_box(vbox.0.width(), vbox.0.height()))
                } else {
                    Some(self.get_view_params())
                }
            })
    }

    pub fn acquire_node(
        &mut self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<AcquiredNode, AcquireError> {
        self.acquired_nodes.acquire(fragment, node_types)
    }

    fn clip_to_node(
        &mut self,
        clip_node: &Option<RsvgNode>,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        if let Some(node) = clip_node {
            let units = node.borrow().get_impl::<ClipPath>().get_units();

            if units == CoordUnits::ObjectBoundingBox && bbox.rect.is_none() {
                // The node being clipped is empty / doesn't have a
                // bounding box, so there's nothing to clip!
                return Ok(());
            }

            let cascaded = CascadedValues::new_from_node(node);

            let transform = if units == CoordUnits::ObjectBoundingBox {
                let bbox_rect = bbox.rect.as_ref().unwrap();

                Some(Transform::new(
                    bbox_rect.width(),
                    0.0,
                    0.0,
                    bbox_rect.height(),
                    bbox_rect.x0,
                    bbox_rect.y0,
                ))
            } else {
                None
            };

            self.with_saved_transform(transform, &mut |dc| {
                let cr = dc.get_cairo_context();

                // here we don't push a layer because we are clipping
                let res = node.draw_children(&cascaded, dc, true);

                cr.clip();

                res
            })
            .and_then(|_bbox|
                // Clipping paths do not contribute to bounding boxes (they should,
                // but we need Real Computational Geometry(tm), so ignore the
                // bbox from the clip path.
                Ok(()))
        } else {
            Ok(())
        }
    }

    fn generate_cairo_mask(
        &mut self,
        mask: &Mask,
        mask_node: &RsvgNode,
        transform: Transform,
        bbox: &BoundingBox,
    ) -> Result<Option<cairo::ImageSurface>, RenderingError> {
        if bbox.rect.is_none() {
            // The node being masked is empty / doesn't have a
            // bounding box, so there's nothing to mask!
            return Ok(None);
        }

        let bbox_rect = bbox.rect.as_ref().unwrap();
        let (bb_x, bb_y) = (bbox_rect.x0, bbox_rect.y0);
        let (bb_w, bb_h) = bbox_rect.size();

        let cascaded = CascadedValues::new_from_node(mask_node);
        let values = cascaded.get();

        let mask_units = mask.get_units();

        let mask_rect = {
            let params = if mask_units == CoordUnits::ObjectBoundingBox {
                self.push_view_box(1.0, 1.0)
            } else {
                self.get_view_params()
            };

            mask.get_rect(&values, &params)
        };

        let mask_transform = mask_node
            .borrow()
            .get_transform()
            .post_transform(&transform);

        let mask_content_surface = self.create_surface_for_toplevel_viewport()?;

        // Use a scope because mask_cr needs to release the
        // reference to the surface before we access the pixels
        {
            let mask_cr = cairo::Context::new(&mask_content_surface);
            mask_cr.set_matrix(mask_transform.into());

            let bbtransform = Transform::new(bb_w, 0.0, 0.0, bb_h, bb_x, bb_y);

            let clip_rect = if mask_units == CoordUnits::ObjectBoundingBox {
                bbtransform.transform_rect(&mask_rect)
            } else {
                mask_rect
            };

            mask_cr.rectangle(
                clip_rect.x0,
                clip_rect.y0,
                clip_rect.width(),
                clip_rect.height(),
            );
            mask_cr.clip();

            self.push_cairo_context(mask_cr);

            let _params = if mask.get_content_units() == CoordUnits::ObjectBoundingBox {
                self.get_cairo_context().transform(bbtransform.into());
                self.push_view_box(1.0, 1.0)
            } else {
                self.get_view_params()
            };

            let res = self.with_discrete_layer(mask_node, values, false, &mut |dc| {
                mask_node.draw_children(&cascaded, dc, false)
            });

            self.pop_cairo_context();

            res?;
        }

        let Opacity(opacity) = values.opacity;

        let mask = SharedImageSurface::wrap(mask_content_surface, SurfaceType::SRgb)?
            .to_mask(opacity)?
            .into_image_surface()?;

        Ok(Some(mask))
    }

    pub fn with_discrete_layer(
        &mut self,
        node: &RsvgNode,
        values: &ComputedValues,
        clipping: bool,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        if clipping {
            draw_fn(self)
        } else {
            self.with_saved_cr(&mut |dc| {
                let clip_uri = values.clip_path.0.get();
                let mask = values.mask.0.get();

                // The `filter` property does not apply to masks.
                let filter = if node.borrow().get_type() == NodeType::Mask {
                    None
                } else {
                    values.filter.0.get()
                };

                let UnitInterval(opacity) = values.opacity.0;

                let affine_at_start = dc.get_transform();

                let (clip_in_user_space, clip_in_object_space) =
                    get_clip_in_user_and_object_space(&mut dc.acquired_nodes, clip_uri);

                // Here we are clipping in user space, so the bbox doesn't matter
                dc.clip_to_node(&clip_in_user_space, &dc.empty_bbox())?;

                let needs_temporary_surface = !(opacity == 1.0
                    && filter.is_none()
                    && mask.is_none()
                    && clip_in_object_space.is_none());

                if needs_temporary_surface {
                    // Compute our assortment of affines

                    let affines = CompositingAffines::new(
                        affine_at_start,
                        dc.initial_transform_with_offset(),
                        dc.cr_stack.len(),
                    );

                    // Create temporary surface and its cr

                    let cr = if filter.is_some() {
                        cairo::Context::new(&*dc.create_surface_for_toplevel_viewport()?)
                    } else {
                        cairo::Context::new(
                            &dc.create_similar_surface_for_toplevel_viewport(&dc.cr.get_target())?,
                        )
                    };

                    cr.set_matrix(affines.for_temporary_surface.into());

                    dc.push_cairo_context(cr);

                    // Draw!

                    let mut res = draw_fn(dc);

                    let bbox = if let Ok(ref bbox) = res {
                        *bbox
                    } else {
                        BoundingBox::new().with_transform(affines.for_temporary_surface)
                    };

                    // Filter

                    let source_surface = if let Some(filter_uri) = filter {
                        // The target surface has multiple references.
                        // We need to copy it to a new surface to have a unique
                        // reference to be able to safely access the pixel data.
                        let child_surface = SharedImageSurface::copy_from_surface(
                            &cairo::ImageSurface::try_from(dc.cr.get_target()).unwrap(),
                        )?;

                        let img_surface = dc
                            .run_filter(filter_uri, node, values, child_surface, bbox)?
                            .into_image_surface()?;

                        // turn ImageSurface into a Surface
                        (*img_surface).clone()
                    } else {
                        dc.cr.get_target()
                    };

                    dc.pop_cairo_context();

                    // Set temporary surface as source

                    dc.cr.set_matrix(affines.compositing.into());
                    dc.cr.set_source_surface(&source_surface, 0.0, 0.0);

                    // Clip

                    dc.cr.set_matrix(affines.outside_temporary_surface.into());
                    let _: () = dc.clip_to_node(&clip_in_object_space, &bbox)?;

                    // Mask

                    if let Some(fragment) = mask {
                        if let Ok(acquired) = dc.acquire_node(fragment, &[NodeType::Mask]) {
                            let mask_node = acquired.get();

                            res = res.and_then(|bbox| {
                                dc.generate_cairo_mask(
                                    &mask_node.borrow().get_impl::<Mask>(),
                                    &mask_node,
                                    affines.for_temporary_surface,
                                    &bbox,
                                )
                                .and_then(|mask_surf| {
                                    if let Some(surf) = mask_surf {
                                        dc.cr.set_matrix(affines.compositing.into());
                                        dc.cr.mask_surface(&surf, 0.0, 0.0);
                                    }
                                    Ok(())
                                })
                                .map(|_: ()| bbox)
                            });
                        } else {
                            rsvg_log!(
                                "element {} references nonexistent mask \"{}\"",
                                node,
                                fragment
                            );
                        }
                    } else {
                        // No mask, so composite the temporary surface

                        dc.cr.set_matrix(affines.compositing.into());

                        if opacity < 1.0 {
                            dc.cr.paint_with_alpha(opacity);
                        } else {
                            dc.cr.paint();
                        }
                    }

                    dc.cr.set_matrix(affine_at_start.into());

                    res
                } else {
                    draw_fn(dc)
                }
            })
        }
    }

    fn initial_transform_with_offset(&self) -> Transform {
        self.initial_transform
            .pre_translate(self.rect.x0, self.rect.y0)
    }

    /// Saves the current transform, applies a new transform if specified,
    /// runs the draw_fn, and restores the original transform
    ///
    /// This is slightly cheaper than a `cr.save()` / `cr.restore()`
    /// pair, but more importantly, it does not reset the whole
    /// graphics state, i.e. it leaves a clipping path in place if it
    /// was set by the `draw_fn`.
    pub fn with_saved_transform(
        &mut self,
        transform: Option<Transform>,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        let orig_transform = self.get_transform();

        if let Some(t) = transform {
            self.cr.transform(t.into());
        }

        let res = draw_fn(self);

        self.cr.set_matrix(orig_transform.into());

        if let Ok(bbox) = res {
            let mut res_bbox = BoundingBox::new().with_transform(orig_transform);
            res_bbox.insert(&bbox);
            Ok(res_bbox)
        } else {
            res
        }
    }

    /// if a rectangle is specified, clips and runs the draw_fn, otherwise simply run the draw_fn
    pub fn with_clip_rect(
        &mut self,
        clip: Option<Rect>,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        if let Some(rect) = clip {
            self.cr.save();
            self.cr.rectangle(rect.x0, rect.y0, rect.width(), rect.height());
            self.cr.clip();
        }

        let res = draw_fn(self);

        if clip.is_some() {
            self.cr.restore();
        }

        res
    }

    /// Saves the current Cairo context, runs the draw_fn, and restores the context
    pub fn with_saved_cr(
        &mut self,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        self.cr.save();
        let res = draw_fn(self);
        self.cr.restore();
        res
    }

    /// Wraps the draw_fn in a link to the given target
    pub fn with_link_tag(
        &mut self,
        link_target: &str,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        const CAIRO_TAG_LINK: &str = "Link";

        let attributes = format!("uri='{}'", escape_link_target(link_target));

        let cr = self.get_cairo_context();
        cr.tag_begin(CAIRO_TAG_LINK, &attributes);

        let res = draw_fn(self);

        cr.tag_end(CAIRO_TAG_LINK);

        res
    }

    fn run_filter(
        &mut self,
        filter_uri: &Fragment,
        node: &RsvgNode,
        values: &ComputedValues,
        child_surface: SharedImageSurface,
        node_bbox: BoundingBox,
    ) -> Result<SharedImageSurface, RenderingError> {
        match self.acquire_node(filter_uri, &[NodeType::Filter]) {
            Ok(acquired) => {
                let filter_node = acquired.get();

                if !filter_node.borrow().is_in_error() {
                    // FIXME: deal with out of memory here
                    filters::render(&filter_node, values, child_surface, self, node_bbox)
                } else {
                    Ok(child_surface)
                }
            }

            Err(_) => {
                rsvg_log!(
                    "element {} will not be rendered since its filter \"{}\" was not found",
                    node,
                    filter_uri,
                );

                // Non-existing filters must act as null filters (that is, an
                // empty surface is returned).
                Ok(SharedImageSurface::empty(
                    child_surface.width(),
                    child_surface.height(),
                    child_surface.surface_type(),
                )?)
            }
        }
    }

    fn set_color(
        &self,
        color: cssparser::Color,
        opacity: UnitInterval,
        current_color: cssparser::RGBA,
    ) {
        let rgba = match color {
            cssparser::Color::RGBA(rgba) => rgba,
            cssparser::Color::CurrentColor => current_color,
        };

        let UnitInterval(o) = opacity;
        self.get_cairo_context().set_source_rgba(
            f64::from(rgba.red_f32()),
            f64::from(rgba.green_f32()),
            f64::from(rgba.blue_f32()),
            f64::from(rgba.alpha_f32()) * o,
        );
    }

    pub fn set_source_paint_server(
        &mut self,
        ps: &PaintServer,
        opacity: UnitInterval,
        bbox: &BoundingBox,
        current_color: cssparser::RGBA,
    ) -> Result<bool, RenderingError> {
        match *ps {
            PaintServer::Iri {
                ref iri,
                ref alternate,
            } => {
                let mut had_paint_server = false;

                match acquire_paint_server(&mut self.acquired_nodes, iri) {
                    Ok(acquired) => {
                        let node = acquired.get();

                        had_paint_server = match node.borrow().get_type() {
                            NodeType::LinearGradient => node
                                .borrow()
                                .get_impl::<LinearGradient>()
                                .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                            NodeType::RadialGradient => node
                                .borrow()
                                .get_impl::<RadialGradient>()
                                .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                            NodeType::Pattern => node
                                .borrow()
                                .get_impl::<Pattern>()
                                .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                            _ => unreachable!(),
                        }
                    }

                    Err(AcquireError::MaxReferencesExceeded) => {
                        return Err(RenderingError::InstancingLimit);
                    }

                    Err(_) => (),
                }

                if !had_paint_server && alternate.is_some() {
                    self.set_color(alternate.unwrap(), opacity, current_color);
                    had_paint_server = true;
                } else {
                    rsvg_log!(
                        "pattern \"{}\" was not found and there was no fallback alternate",
                        iri
                    );
                }

                Ok(had_paint_server)
            }

            PaintServer::SolidColor(color) => {
                self.set_color(color, opacity, current_color);
                Ok(true)
            }

            PaintServer::None => Ok(false),
        }
    }

    pub fn setup_cr_for_stroke(&self, cr: &cairo::Context, values: &ComputedValues) {
        let params = self.get_view_params();

        cr.set_line_width(values.stroke_width.0.normalize(values, &params));
        cr.set_miter_limit(values.stroke_miterlimit.0);
        cr.set_line_cap(cairo::LineCap::from(values.stroke_line_cap));
        cr.set_line_join(cairo::LineJoin::from(values.stroke_line_join));

        if let StrokeDasharray(Dasharray::Array(ref dashes)) = values.stroke_dasharray {
            let normalized_dashes: Vec<f64> = dashes
                .iter()
                .map(|l| l.normalize(values, &params))
                .collect();

            let total_length = normalized_dashes.iter().fold(0.0, |acc, &len| acc + len);

            if total_length > 0.0 {
                let offset = values.stroke_dashoffset.0.normalize(values, &params);
                cr.set_dash(&normalized_dashes, offset);
            } else {
                cr.set_dash(&[], 0.0);
            }
        }
    }

    pub fn stroke_and_fill(
        &mut self,
        cr: &cairo::Context,
        values: &ComputedValues,
    ) -> Result<BoundingBox, RenderingError> {
        cr.set_antialias(cairo::Antialias::from(values.shape_rendering));

        self.setup_cr_for_stroke(cr, values);

        // Update the bbox in the rendering context.  Below, we actually set the
        // fill/stroke patterns on the cairo_t.  That process requires the
        // rendering context to have an updated bbox; for example, for the
        // coordinate system in patterns.
        let bbox = compute_stroke_and_fill_box(cr, values);

        let current_color = values.color.0;

        let res = self
            .set_source_paint_server(&values.fill.0, values.fill_opacity.0, &bbox, current_color)
            .and_then(|had_paint_server| {
                if had_paint_server {
                    if values.stroke.0 == PaintServer::None {
                        cr.fill();
                    } else {
                        cr.fill_preserve();
                    }
                }

                Ok(())
            })
            .and_then(|_| {
                self.set_source_paint_server(
                    &values.stroke.0,
                    values.stroke_opacity.0,
                    &bbox,
                    current_color,
                )
                .and_then(|had_paint_server| {
                    if had_paint_server {
                        cr.stroke();
                    }
                    Ok(())
                })
            });

        // clear the path in case stroke == fill == None; otherwise
        // we leave it around from computing the bounding box
        cr.new_path();

        res.and_then(|_: ()| Ok(bbox))
    }

    pub fn draw_path(
        &mut self,
        builder: &PathBuilder,
        node: &RsvgNode,
        values: &ComputedValues,
        markers: Markers,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if !builder.is_empty() {
            let bbox = self.with_discrete_layer(node, values, clipping, &mut |dc| {
                let cr = dc.get_cairo_context();

                builder.to_cairo(&cr)?;

                if clipping {
                    cr.set_fill_rule(cairo::FillRule::from(values.clip_rule));
                    Ok(dc.empty_bbox())
                } else {
                    cr.set_fill_rule(cairo::FillRule::from(values.fill_rule));
                    dc.stroke_and_fill(&cr, values)
                }
            })?;

            if markers == Markers::Yes {
                marker::render_markers_for_path_builder(builder, self, values, clipping)?;
            }

            Ok(bbox)
        } else {
            Ok(self.empty_bbox())
        }
    }

    pub fn get_snapshot(
        &self,
        width: i32,
        height: i32,
    ) -> Result<SharedImageSurface, cairo::Status> {
        // TODO: as far as I can tell this should not render elements past the last (topmost) one
        // with enable-background: new (because technically we shouldn't have been caching them).
        // Right now there are no enable-background checks whatsoever.
        //
        // Addendum: SVG 2 has deprecated the enable-background property, and replaced it with an
        // "isolation" property from the CSS Compositing and Blending spec.
        //
        // Deprecation:
        //   https://www.w3.org/TR/filter-effects-1/#AccessBackgroundImage
        //
        // BackgroundImage, BackgroundAlpha in the "in" attribute of filter primitives:
        //   https://www.w3.org/TR/filter-effects-1/#attr-valuedef-in-backgroundimage
        //
        // CSS Compositing and Blending, "isolation" property:
        //   https://www.w3.org/TR/compositing-1/#isolation
        let mut surface = ExclusiveImageSurface::new(width, height, SurfaceType::SRgb)?;

        surface.draw(&mut |cr| {
            for (depth, draw) in self.cr_stack.iter().enumerate() {
                let affines = CompositingAffines::new(
                    Transform::from(draw.get_matrix()),
                    self.initial_transform_with_offset(),
                    depth,
                );

                cr.set_matrix(affines.for_snapshot.into());
                cr.set_source_surface(&draw.get_target(), 0.0, 0.0);
                cr.paint();
            }

            Ok(())
        })?;

        surface.share()
    }

    pub fn lookup_image(&self, href: &str) -> Result<SharedImageSurface, RenderingError> {
        self.document
            .lookup_image(href)
            .map_err(|_| RenderingError::InvalidHref)
    }

    pub fn draw_node_to_surface(
        &mut self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        affine: Transform,
        width: i32,
        height: i32,
    ) -> Result<SharedImageSurface, RenderingError> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;

        let save_cr = self.cr.clone();
        let save_rect = self.rect;

        {
            let cr = cairo::Context::new(&surface);
            cr.set_matrix(affine.into());

            self.cr = cr;

            self.rect = Rect::from_size(f64::from(width), f64::from(height));

            let _ = self.draw_node_from_stack(cascaded, node, false)?;
        }

        self.cr = save_cr;
        self.rect = save_rect;

        Ok(SharedImageSurface::wrap(surface, SurfaceType::SRgb)?)
    }

    pub fn draw_node_from_stack(
        &mut self,
        cascaded: &CascadedValues<'_>,
        node: &RsvgNode,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let stack_top = self.drawsub_stack.pop();

        let draw = if let Some(ref top) = stack_top {
            top == node
        } else {
            true
        };

        let values = cascaded.get();
        let res = if draw && values.is_visible() {
            node.draw(cascaded, self, clipping)
        } else {
            Ok(self.empty_bbox())
        };

        if let Some(top) = stack_top {
            self.drawsub_stack.push(top);
        }

        res
    }

    pub fn draw_from_use_node(
        &mut self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let node_data = node.borrow();
        let use_ = node_data.get_impl::<Use>();

        // <use> is an element that is used directly, unlike
        // <pattern>, which is used through a fill="url(#...)"
        // reference.  However, <use> will always reference another
        // element, potentially itself or an ancestor of itself (or
        // another <use> which references the first one, etc.).  So,
        // we acquire the <use> element itself so that circular
        // references can be caught.
        let _self_acquired = self.acquired_nodes.acquire_ref(node).map_err(|e| {
            if let AcquireError::CircularReference(_) = e {
                rsvg_log!("circular reference in element {}", node);
                RenderingError::CircularReference
            } else {
                unreachable!();
            }
        })?;

        let link = use_.get_link();
        if link.is_none() {
            return Ok(self.empty_bbox());
        }

        let acquired = match self.acquire_node(link.unwrap(), &[]) {
            Ok(acquired) => acquired,

            Err(AcquireError::CircularReference(node)) => {
                rsvg_log!("circular reference in element {}", node);
                return Err(RenderingError::CircularReference);
            }

            Err(AcquireError::MaxReferencesExceeded) => {
                return Err(RenderingError::InstancingLimit);
            }

            Err(AcquireError::InvalidLinkType(_)) => unreachable!(),

            Err(AcquireError::LinkNotFound(fragment)) => {
                rsvg_log!("element {} references nonexistent \"{}\"", node, fragment);
                return Ok(self.empty_bbox());
            }
        };

        let values = cascaded.get();
        let params = self.get_view_params();
        let use_rect = use_.get_rect(values, &params);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if use_rect.is_empty() {
            return Ok(self.empty_bbox());
        }

        let child = acquired.get();

        if child.borrow().get_type() != NodeType::Symbol {
            let cr = self.get_cairo_context();
            cr.translate(use_rect.x0, use_rect.y0);

            self.with_discrete_layer(node, values, clipping, &mut |dc| {
                dc.draw_node_from_stack(
                    &CascadedValues::new_from_values(&child, values),
                    &child,
                    clipping,
                )
            })
        } else {
            let node_data = child.borrow();
            let symbol = node_data.get_impl::<Symbol>();

            let clip_mode = if !values.is_overflow()
                || (values.overflow == Overflow::Visible && child.borrow().is_overflow())
            {
                Some(ClipMode::ClipToVbox)
            } else {
                None
            };

            self.with_discrete_layer(node, values, clipping, &mut |dc| {
                let _params = dc.push_new_viewport(
                    symbol.get_viewbox(),
                    use_rect,
                    symbol.get_preserve_aspect_ratio(),
                    clip_mode,
                );

                child.draw_children(
                    &CascadedValues::new_from_values(&child, values),
                    dc,
                    clipping,
                )
            })
        }
    }
}

#[derive(Debug)]
struct CompositingAffines {
    pub outside_temporary_surface: Transform,
    pub initial: Transform,
    pub for_temporary_surface: Transform,
    pub compositing: Transform,
    pub for_snapshot: Transform,
}

impl CompositingAffines {
    fn new(current: Transform, initial: Transform, cr_stack_depth: usize) -> CompositingAffines {
        let is_topmost_temporary_surface = cr_stack_depth == 0;

        let initial_inverse = initial.invert().unwrap();

        let outside_temporary_surface = if is_topmost_temporary_surface {
            current
        } else {
            current.post_transform(&initial_inverse)
        };

        let (scale_x, scale_y) = initial.transform_distance(1.0, 1.0);

        let for_temporary_surface = if is_topmost_temporary_surface {
            current
                .post_transform(&initial_inverse)
                .post_scale(scale_x, scale_y)
        } else {
            current
        };

        let compositing = if is_topmost_temporary_surface {
            initial.pre_scale(1.0 / scale_x, 1.0 / scale_y)
        } else {
            Transform::identity()
        };

        let for_snapshot = compositing.invert().unwrap();

        CompositingAffines {
            outside_temporary_surface,
            initial,
            for_temporary_surface,
            compositing,
            for_snapshot,
        }
    }
}

// Returns (clip_in_user_space, clip_in_object_space), both Option<RsvgNode>
fn get_clip_in_user_and_object_space(
    acquired_nodes: &mut AcquiredNodes,
    clip_uri: Option<&Fragment>,
) -> (Option<RsvgNode>, Option<RsvgNode>) {
    clip_uri
        .and_then(|fragment| acquired_nodes.acquire(fragment, &[NodeType::ClipPath]).ok())
        .and_then(|acquired| {
            let clip_node = acquired.get().clone();

            let units = clip_node.borrow().get_impl::<ClipPath>().get_units();

            match units {
                CoordUnits::UserSpaceOnUse => Some((Some(clip_node), None)),
                CoordUnits::ObjectBoundingBox => Some((None, Some(clip_node))),
            }
        })
        .unwrap_or((None, None))
}

fn acquire_paint_server(
    acquired_nodes: &mut AcquiredNodes,
    fragment: &Fragment,
) -> Result<AcquiredNode, AcquireError> {
    acquired_nodes.acquire(
        fragment,
        &[
            NodeType::LinearGradient,
            NodeType::RadialGradient,
            NodeType::Pattern,
        ],
    )
}

fn compute_stroke_and_fill_box(cr: &cairo::Context, values: &ComputedValues) -> BoundingBox {
    let affine = Transform::from(cr.get_matrix());

    let mut bbox = BoundingBox::new().with_transform(affine);

    // Dropping the precision of cairo's bezier subdivision, yielding 2x
    // _rendering_ time speedups, are these rather expensive operations
    // really needed here? */
    let backup_tolerance = cr.get_tolerance();
    cr.set_tolerance(1.0);

    // Bounding box for fill
    //
    // Unlike the case for stroke, for fills we always compute the bounding box.
    // In GNOME we have SVGs for symbolic icons where each icon has a bounding
    // rectangle with no fill and no stroke, and inside it there are the actual
    // paths for the icon's shape.  We need to be able to compute the bounding
    // rectangle's extents, even when it has no fill nor stroke.

    let (x0, y0, x1, y1) = cr.fill_extents();
    let fb = BoundingBox::new()
        .with_transform(affine)
        .with_ink_rect(Rect::new(x0, y0, x1, y1));
    bbox.insert(&fb);

    // Bounding box for stroke

    if values.stroke.0 != PaintServer::None {
        let (x0, y0, x1, y1) = cr.stroke_extents();
        let sb = BoundingBox::new()
            .with_transform(affine)
            .with_ink_rect(Rect::new(x0, y0, x1, y1));
        bbox.insert(&sb);
    }

    // objectBoundingBox

    let (x0, y0, x1, y1) = cr.path_extents();
    let ob = BoundingBox::new()
        .with_transform(affine)
        .with_rect(Rect::new(x0, y0, x1, y1));
    bbox.insert(&ob);

    // restore tolerance

    cr.set_tolerance(backup_tolerance);

    bbox
}

/// escape quotes and backslashes with backslash
fn escape_link_target(value: &str) -> Cow<'_, str> {
    static REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"['\\]").unwrap());

    REGEX.replace_all(value, |caps: &Captures<'_>| {
        match caps.get(0).unwrap().as_str() {
            "'" => "\\'".to_owned(),
            "\\" => "\\\\".to_owned(),
            _ => unreachable!(),
        }
    })
}

impl From<StrokeLinejoin> for cairo::LineJoin {
    fn from(j: StrokeLinejoin) -> cairo::LineJoin {
        match j {
            StrokeLinejoin::Miter => cairo::LineJoin::Miter,
            StrokeLinejoin::Round => cairo::LineJoin::Round,
            StrokeLinejoin::Bevel => cairo::LineJoin::Bevel,
        }
    }
}

impl From<StrokeLinecap> for cairo::LineCap {
    fn from(j: StrokeLinecap) -> cairo::LineCap {
        match j {
            StrokeLinecap::Butt => cairo::LineCap::Butt,
            StrokeLinecap::Round => cairo::LineCap::Round,
            StrokeLinecap::Square => cairo::LineCap::Square,
        }
    }
}

impl From<ClipRule> for cairo::FillRule {
    fn from(c: ClipRule) -> cairo::FillRule {
        match c {
            ClipRule::NonZero => cairo::FillRule::Winding,
            ClipRule::EvenOdd => cairo::FillRule::EvenOdd,
        }
    }
}

impl From<FillRule> for cairo::FillRule {
    fn from(f: FillRule) -> cairo::FillRule {
        match f {
            FillRule::NonZero => cairo::FillRule::Winding,
            FillRule::EvenOdd => cairo::FillRule::EvenOdd,
        }
    }
}

impl From<ShapeRendering> for cairo::Antialias {
    fn from(sr: ShapeRendering) -> cairo::Antialias {
        match sr {
            ShapeRendering::Auto | ShapeRendering::GeometricPrecision => cairo::Antialias::Default,
            ShapeRendering::OptimizeSpeed | ShapeRendering::CrispEdges => cairo::Antialias::None,
        }
    }
}

pub struct AcquiredNode {
    stack: Option<Rc<RefCell<NodeStack>>>,
    node: RsvgNode,
}

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        if let Some(ref stack) = self.stack {
            let mut stack = stack.borrow_mut();
            let last = stack.pop().unwrap();
            assert!(last == self.node);
        }
    }
}

impl AcquiredNode {
    pub fn get(&self) -> &RsvgNode {
        &self.node
    }
}

/// This helper struct is used when looking up urls to other nodes.
/// Its methods do recursion checking and thereby avoid infinite loops.
///
/// Malformed SVGs, for example, may reference a marker by its IRI, but
/// the object referenced by the IRI is not a marker.
///
/// Note that if you acquire a node, you have to release it before trying to
/// acquire it again.  If you acquire a node "#foo" and don't release it before
/// trying to acquire "foo" again, you will obtain a None the second time.
struct AcquiredNodes<'i> {
    document: &'i Document,
    num_elements_acquired: usize,
    node_stack: Rc<RefCell<NodeStack>>,
}

impl<'i> AcquiredNodes<'i> {
    fn new(document: &Document) -> AcquiredNodes {
        AcquiredNodes {
            document,
            num_elements_acquired: 0,
            node_stack: Rc::new(RefCell::new(NodeStack::new())),
        }
    }

    fn lookup_node(
        &self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<RsvgNode, AcquireError> {
        let node = self.document.lookup(fragment).map_err(|_| {
            // FIXME: callers shouldn't have to know that get_node() can initiate a file load.
            // Maybe we should have the following stages:
            //   - load main SVG XML
            //
            //   - load secondary SVG XML and other files like images; all document::Resources and
            //     document::Images loaded
            //
            //   - Now that all files are loaded, resolve URL references
            AcquireError::LinkNotFound(fragment.clone())
        })?;

        if node_types.is_empty() {
            Ok(node)
        } else {
            let node_type = node.borrow().get_type();
            if node_types.iter().find(|&&t| t == node_type).is_some() {
                Ok(node)
            } else {
                Err(AcquireError::InvalidLinkType(fragment.clone()))
            }
        }
    }

    /// Acquires a node.
    /// Specify `node_types` when expecting the node to be of a particular type,
    /// or use an empty slice for `node_types` if you want a node of any type.
    /// Nodes acquired by this function must be released in reverse acquiring order.
    fn acquire(
        &mut self,
        fragment: &Fragment,
        node_types: &[NodeType],
    ) -> Result<AcquiredNode, AcquireError> {
        self.num_elements_acquired += 1;

        // This is a mitigation for SVG files that try to instance a huge number of
        // elements via <use>, recursive patterns, etc.  See limits.rs for details.
        if self.num_elements_acquired > limits::MAX_REFERENCED_ELEMENTS {
            return Err(AcquireError::MaxReferencesExceeded);
        }

        let node = self.lookup_node(fragment, node_types)?;

        if node_is_accessed_by_reference(&node) {
            self.acquire_ref(&node)
        } else {
            Ok(AcquiredNode { stack: None, node })
        }
    }

    fn acquire_ref(&self, node: &RsvgNode) -> Result<AcquiredNode, AcquireError> {
        if self.node_stack.borrow().contains(&node) {
            Err(AcquireError::CircularReference(node.clone()))
        } else {
            self.node_stack.borrow_mut().push(&node);
            Ok(AcquiredNode {
                stack: Some(self.node_stack.clone()),
                node: node.clone(),
            })
        }
    }
}

// Returns whether a node of a particular type is only accessed by reference
// from other nodes' atributes.  The node could in turn cause other nodes
// to get referenced, potentially causing reference cycles.
fn node_is_accessed_by_reference(node: &RsvgNode) -> bool {
    use NodeType::*;

    match node.borrow().get_type() {
        ClipPath | Filter | LinearGradient | Marker | Mask | Pattern | RadialGradient => true,

        _ => false,
    }
}

/// Keeps a stack of nodes and can check if a certain node is contained in the stack
///
/// Sometimes parts of the code cannot plainly use the implicit stack of acquired
/// nodes as maintained by DrawingCtx::acquire_node(), and they must keep their
/// own stack of nodes to test for reference cycles.  NodeStack can be used to do that.
pub struct NodeStack(Vec<RsvgNode>);

impl NodeStack {
    pub fn new() -> NodeStack {
        NodeStack(Vec::new())
    }

    pub fn push(&mut self, node: &RsvgNode) {
        self.0.push(node.clone());
    }

    pub fn pop(&mut self) -> Option<RsvgNode> {
        self.0.pop()
    }

    pub fn contains(&self, node: &RsvgNode) -> bool {
        self.0.iter().find(|n| **n == *node).is_some()
    }
}
