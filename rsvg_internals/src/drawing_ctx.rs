//! The main context structure which drives the drawing process.

use float_cmp::approx_eq;
use once_cell::sync::Lazy;
use pango::FontMapExt;
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
use crate::document::AcquiredNodes;
use crate::dpi::Dpi;
use crate::element::Element;
use crate::error::{AcquireError, RenderingError};
use crate::filter::FilterValue;
use crate::filters;
use crate::float_eq_cairo::ApproxEqCairo;
use crate::gradient::{Gradient, GradientUnits, GradientVariant, SpreadMethod};
use crate::marker;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::paint_server::{PaintServer, PaintSource};
use crate::path_builder::*;
use crate::pattern::{PatternContentUnits, PatternUnits, ResolvedPattern};
use crate::properties::ComputedValues;
use crate::property_defs::{
    ClipRule, FillRule, Filter, MixBlendMode, Opacity, Overflow, PaintTarget, ShapeRendering,
    StrokeDasharray, StrokeLinecap, StrokeLinejoin, TextRendering,
};
use crate::rect::Rect;
use crate::shapes::Markers;
use crate::structure::Mask;
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
    pub dpi: Dpi,
    pub vbox: ViewBox,
    viewport_stack: Option<Weak<RefCell<Vec<Viewport>>>>,
}

impl ViewParams {
    pub fn new(dpi: Dpi, view_box_width: f64, view_box_height: f64) -> ViewParams {
        ViewParams {
            dpi,
            vbox: ViewBox::from(Rect::from_size(view_box_width, view_box_height)),
            viewport_stack: None,
        }
    }
}

impl Drop for ViewParams {
    fn drop(&mut self) {
        if let Some(ref weak_stack) = self.viewport_stack {
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

/// Set path on the cairo context, or clear it.
/// This helper object keeps track whether the path has been set already,
/// so that it isn't recalculated every so often.
struct PathHelper<'a> {
    cr: &'a cairo::Context,
    path: &'a Path,
    is_square_linecap: bool,
    has_path: Option<bool>,
}

impl<'a> PathHelper<'a> {
    pub fn new(cr: &'a cairo::Context, path: &'a Path, values: &ComputedValues) -> Self {
        PathHelper {
            cr,
            path,
            is_square_linecap: values.stroke_line_cap() == StrokeLinecap::Square,
            has_path: None,
        }
    }

    pub fn set(&mut self) -> Result<(), cairo::Status> {
        match self.has_path {
            Some(false) | None => {
                self.has_path = Some(true);
                self.path.to_cairo(self.cr, self.is_square_linecap)
            }
            Some(true) => Ok(()),
        }
    }

    pub fn unset(&mut self) {
        match self.has_path {
            Some(true) | None => {
                self.has_path = Some(false);
                self.cr.new_path();
            }
            Some(false) => {}
        }
    }
}

#[derive(Copy, Clone)]
struct Viewport {
    /// The viewport's coordinate system, or "user coordinate system" in SVG terms.
    transform: Transform,

    /// Corners of the current coordinate space.
    vbox: ViewBox,
}

pub struct DrawingCtx {
    initial_viewport: Viewport,

    dpi: Dpi,

    cr_stack: Vec<cairo::Context>,
    cr: cairo::Context,

    viewport_stack: Rc<RefCell<Vec<Viewport>>>,

    drawsub_stack: Vec<Node>,

    measuring: bool,
    testing: bool,
}

pub enum DrawingMode {
    LimitToStack { node: Node, root: Node },

    OnlyNode(Node),
}

/// The toplevel drawing routine.
///
/// This creates a DrawingCtx internally and starts drawing at the specified `node`.
pub fn draw_tree(
    mode: DrawingMode,
    cr: &cairo::Context,
    viewport: Rect,
    dpi: Dpi,
    measuring: bool,
    testing: bool,
    acquired_nodes: &mut AcquiredNodes,
) -> Result<BoundingBox, RenderingError> {
    let (drawsub_stack, node) = match mode {
        DrawingMode::LimitToStack { node, root } => {
            (node.ancestors().map(|n| n.clone()).collect(), root)
        }

        DrawingMode::OnlyNode(node) => (Vec::new(), node),
    };

    let cascaded = CascadedValues::new_from_node(&node);

    let transform = Transform::from(cr.get_matrix());
    let mut bbox = BoundingBox::new().with_transform(transform);

    let mut draw_ctx = DrawingCtx::new(cr, viewport, dpi, measuring, testing, drawsub_stack);

    let content_bbox = draw_ctx.draw_node_from_stack(&node, acquired_nodes, &cascaded, false)?;

    bbox.insert(&content_bbox);

    Ok(bbox)
}

impl DrawingCtx {
    fn new(
        cr: &cairo::Context,
        viewport: Rect,
        dpi: Dpi,
        measuring: bool,
        testing: bool,
        drawsub_stack: Vec<Node>,
    ) -> DrawingCtx {
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

        // Translate so (0, 0) is at the viewport's upper-left corner.
        cr.translate(viewport.x0, viewport.y0);
        let transform = Transform::from(cr.get_matrix());

        // Per the spec, so the viewport has (0, 0) as upper-left.
        let viewport = viewport.translate((-viewport.x0, -viewport.y0));
        let vbox = ViewBox::from(viewport);

        let initial_viewport = Viewport { transform, vbox };

        let mut viewport_stack = Vec::new();
        viewport_stack.push(initial_viewport);

        DrawingCtx {
            initial_viewport,
            dpi,
            cr_stack: Vec::new(),
            cr: cr.clone(),
            viewport_stack: Rc::new(RefCell::new(viewport_stack)),
            drawsub_stack,
            measuring,
            testing,
        }
    }

    pub fn toplevel_viewport(&self) -> Rect {
        *self.initial_viewport.vbox
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring
    }

    fn get_transform(&self) -> Transform {
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
    fn set_cairo_context(&mut self, cr: &cairo::Context) {
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
        let rect = self.toplevel_viewport();

        let (viewport_width, viewport_height) = (rect.width(), rect.height());

        let (width, height) = self
            .initial_viewport
            .transform
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

    fn get_top_viewport(&self) -> Viewport {
        let viewport_stack = self.viewport_stack.borrow();
        *viewport_stack
            .last()
            .expect("viewport_stack must never be empty!")
    }

    pub fn push_coord_units(&self, units: CoordUnits) -> ViewParams {
        match units {
            CoordUnits::ObjectBoundingBox => self.push_view_box(1.0, 1.0),

            CoordUnits::UserSpaceOnUse => {
                // Duplicate the topmost viewport;
                let viewport = self.get_top_viewport();
                self.push_viewport(viewport)
            }
        }
    }

    /// Gets the viewport that was last pushed with `push_view_box()`.
    pub fn get_view_params(&self) -> ViewParams {
        let viewport = self.get_top_viewport();

        ViewParams {
            dpi: self.dpi,
            vbox: viewport.vbox,
            viewport_stack: None,
        }
    }

    fn push_viewport(&self, viewport: Viewport) -> ViewParams {
        let vbox = viewport.vbox;

        self.viewport_stack.borrow_mut().push(viewport);

        ViewParams {
            dpi: self.dpi,
            vbox,
            viewport_stack: Some(Rc::downgrade(&self.viewport_stack)),
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
        let Viewport { transform, .. } = self.get_top_viewport();

        let vbox = ViewBox::from(Rect::from_size(width, height));
        self.push_viewport(Viewport { transform, vbox })
    }

    /// Creates a new coordinate space inside a viewport and sets a clipping rectangle.
    ///
    /// Note that this actually changes the `draw_ctx.cr`'s transformation to match
    /// the new coordinate space, but the old one is not restored after the
    /// result's `ViewParams` is dropped.  Thus, this function must be called
    /// inside `draw_ctx.with_saved_cr` or `draw_ctx.with_discrete_layer`.
    pub fn push_new_viewport(
        &self,
        vbox: Option<ViewBox>,
        viewport: Rect,
        preserve_aspect_ratio: AspectRatio,
        clip_mode: Option<ClipMode>,
    ) -> Option<ViewParams> {
        if let Some(ClipMode::ClipToViewport) = clip_mode {
            clip_to_rectangle(&self.cr, &viewport);
        }

        preserve_aspect_ratio
            .viewport_to_viewbox_transform(vbox, &viewport)
            .unwrap_or_else(|_e: ()| {
                match vbox {
                    None => unreachable!(
                        "viewport_to_viewbox_transform only returns errors when vbox != None"
                    ),
                    Some(v) => {
                        rsvg_log!(
                            "ignoring viewBox ({}, {}, {}, {}) since it is not usable",
                            v.x0,
                            v.y0,
                            v.width(),
                            v.height()
                        );
                    }
                }
                None
            })
            .map(|t| {
                self.cr.transform(t.into());

                if let Some(vbox) = vbox {
                    if let Some(ClipMode::ClipToVbox) = clip_mode {
                        clip_to_rectangle(&self.cr, &*vbox);
                    }
                }

                let top_viewport = self.get_top_viewport();

                self.push_viewport(Viewport {
                    transform: top_viewport.transform.post_transform(&t),
                    vbox: vbox.unwrap_or(top_viewport.vbox),
                })
            })
    }

    fn clip_to_node(
        &mut self,
        clip_node: &Option<Node>,
        acquired_nodes: &mut AcquiredNodes,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        if clip_node.is_none() {
            return Ok(());
        }

        let node = clip_node.as_ref().unwrap();
        let units = borrow_element_as!(node, ClipPath).get_units();

        if let Ok(transform) = bbox.rect_to_transform(units) {
            let node_transform = node
                .borrow_element()
                .get_transform()
                .post_transform(&transform);

            let cascaded = CascadedValues::new_from_node(node);

            self.with_saved_transform(Some(node_transform), &mut |dc| {
                let cr = dc.cr.clone();

                // here we don't push a layer because we are clipping
                let res = node.draw_children(acquired_nodes, &cascaded, dc, true);

                cr.clip();

                res
            })
            // Clipping paths do not contribute to bounding boxes (they should, but we
            // need Real Computational Geometry(tm), so ignore the bbox from the clip path.
            .map(|_bbox| ())
        } else {
            Ok(())
        }
    }

    fn generate_cairo_mask(
        &mut self,
        mask: &Mask,
        mask_node: &Node,
        transform: Transform,
        bbox: &BoundingBox,
        acquired_nodes: &mut AcquiredNodes,
    ) -> Result<Option<cairo::ImageSurface>, RenderingError> {
        if bbox.rect.is_none() {
            // The node being masked is empty / doesn't have a
            // bounding box, so there's nothing to mask!
            return Ok(None);
        }

        let bbox_rect = bbox.rect.as_ref().unwrap();

        let cascaded = CascadedValues::new_from_node(mask_node);
        let values = cascaded.get();

        let mask_units = mask.get_units();

        let mask_rect = {
            let params = self.push_coord_units(mask_units);
            mask.get_rect(&values, &params)
        };

        let mask_transform = mask_node
            .borrow_element()
            .get_transform()
            .post_transform(&transform);

        let mask_content_surface = self.create_surface_for_toplevel_viewport()?;

        // Use a scope because mask_cr needs to release the
        // reference to the surface before we access the pixels
        {
            let mask_cr = cairo::Context::new(&mask_content_surface);
            mask_cr.set_matrix(mask_transform.into());

            let bbtransform = Transform::new_unchecked(
                bbox_rect.width(),
                0.0,
                0.0,
                bbox_rect.height(),
                bbox_rect.x0,
                bbox_rect.y0,
            );

            let clip_rect = if mask_units == CoordUnits::ObjectBoundingBox {
                bbtransform.transform_rect(&mask_rect)
            } else {
                mask_rect
            };

            clip_to_rectangle(&mask_cr, &clip_rect);

            if mask.get_content_units() == CoordUnits::ObjectBoundingBox {
                if bbox_rect.is_empty() {
                    return Ok(None);
                }
                assert!(bbtransform.is_invertible());
                mask_cr.transform(bbtransform.into());
            }

            let _params = self.push_coord_units(mask.get_content_units());

            self.push_cairo_context(mask_cr);

            let res = self.with_discrete_layer(
                mask_node,
                acquired_nodes,
                values,
                false,
                &mut |an, dc| mask_node.draw_children(an, &cascaded, dc, false),
            );

            self.pop_cairo_context();

            res?;
        }

        let Opacity(opacity) = values.opacity();

        let mask = SharedImageSurface::wrap(mask_content_surface, SurfaceType::SRgb)?
            .to_mask(opacity)?
            .into_image_surface()?;

        Ok(Some(mask))
    }

    pub fn with_discrete_layer(
        &mut self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        clipping: bool,
        draw_fn: &mut dyn FnMut(
            &mut AcquiredNodes,
            &mut DrawingCtx,
        ) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        if clipping {
            draw_fn(acquired_nodes, self)
        } else {
            self.with_saved_cr(&mut |dc| {
                let clip_path_value = values.clip_path();
                let mask_value = values.mask();

                let clip_uri = clip_path_value.0.get();
                let mask = mask_value.0.get();

                let filters = if node.is_element() {
                    match *node.borrow_element() {
                        Element::Mask(_) => Filter::None,
                        _ => values.filter(),
                    }
                } else {
                    values.filter()
                };

                let UnitInterval(opacity) = values.opacity().0;

                let affine_at_start = dc.get_transform();

                let (clip_in_user_space, clip_in_object_space) =
                    get_clip_in_user_and_object_space(acquired_nodes, clip_uri);

                // Here we are clipping in user space, so the bbox doesn't matter
                dc.clip_to_node(&clip_in_user_space, acquired_nodes, &dc.empty_bbox())?;

                let is_opaque = approx_eq!(f64, opacity, 1.0);
                let needs_temporary_surface = !(is_opaque
                    && filters == Filter::None
                    && mask.is_none()
                    && values.mix_blend_mode() == MixBlendMode::Normal
                    && clip_in_object_space.is_none());

                if needs_temporary_surface {
                    // Compute our assortment of affines

                    let affines = CompositingAffines::new(
                        affine_at_start,
                        dc.initial_transform_with_offset(),
                        dc.cr_stack.len(),
                    );

                    // Create temporary surface and its cr

                    let cr = match filters {
                        Filter::None => cairo::Context::new(
                            &dc.create_similar_surface_for_toplevel_viewport(&dc.cr.get_target())?,
                        ),
                        Filter::List(_) => {
                            cairo::Context::new(&*dc.create_surface_for_toplevel_viewport()?)
                        }
                    };

                    cr.set_matrix(affines.for_temporary_surface.into());

                    dc.push_cairo_context(cr);

                    // Draw!

                    let mut res = draw_fn(acquired_nodes, dc);

                    let bbox = if let Ok(ref bbox) = res {
                        *bbox
                    } else {
                        BoundingBox::new().with_transform(affines.for_temporary_surface)
                    };

                    // Filter
                    let source_surface =
                        dc.run_filters(&filters, acquired_nodes, node, values, bbox)?;

                    dc.pop_cairo_context();

                    // Set temporary surface as source

                    dc.cr.set_matrix(affines.compositing.into());
                    dc.cr.set_source_surface(&source_surface, 0.0, 0.0);

                    // Clip

                    dc.cr.set_matrix(affines.outside_temporary_surface.into());
                    dc.clip_to_node(&clip_in_object_space, acquired_nodes, &bbox)?;

                    // Mask

                    if let Some(fragment) = mask {
                        if let Ok(acquired) = acquired_nodes.acquire(fragment) {
                            let mask_node = acquired.get();

                            match *mask_node.borrow_element() {
                                Element::Mask(ref m) => {
                                    res = res.and_then(|bbox| {
                                        dc.generate_cairo_mask(
                                            &m,
                                            &mask_node,
                                            affines.for_temporary_surface,
                                            &bbox,
                                            acquired_nodes,
                                        )
                                        .map(|mask_surf| {
                                            if let Some(surf) = mask_surf {
                                                dc.cr.set_matrix(affines.compositing.into());
                                                dc.cr.mask_surface(&surf, 0.0, 0.0);
                                            }
                                        })
                                        .map(|_: ()| bbox)
                                    });
                                }
                                _ => {
                                    rsvg_log!(
                                        "element {} references \"{}\" which is not a mask",
                                        node,
                                        fragment
                                    );
                                }
                            }
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
                        dc.cr.set_operator(values.mix_blend_mode().into());

                        if opacity < 1.0 {
                            dc.cr.paint_with_alpha(opacity);
                        } else {
                            dc.cr.paint();
                        }
                    }

                    dc.cr.set_matrix(affine_at_start.into());

                    res
                } else {
                    draw_fn(acquired_nodes, dc)
                }
            })
        }
    }

    fn initial_transform_with_offset(&self) -> Transform {
        let rect = self.toplevel_viewport();

        self.initial_viewport
            .transform
            .pre_translate(rect.x0, rect.y0)
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
            clip_to_rectangle(&self.cr, &rect);
        }

        let res = draw_fn(self);

        if clip.is_some() {
            self.cr.restore();
        }

        res
    }

    /// Run the drawing function with the specified opacity
    pub fn with_alpha(
        &mut self,
        opacity: UnitInterval,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        let res;
        let UnitInterval(o) = opacity;
        if o < 1.0 {
            self.cr.push_group();
            res = draw_fn(self);
            self.cr.pop_group_to_source();
            self.cr.paint_with_alpha(o);
        } else {
            res = draw_fn(self);
        }

        res
    }

    /// Saves the current Cairo context, runs the draw_fn, and restores the context
    fn with_saved_cr(
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

        let cr = self.cr.clone();
        cr.tag_begin(CAIRO_TAG_LINK, &attributes);

        let res = draw_fn(self);

        cr.tag_end(CAIRO_TAG_LINK);

        res
    }

    fn run_filters(
        &mut self,
        filters: &Filter,
        acquired_nodes: &mut AcquiredNodes,
        node: &Node,
        values: &ComputedValues,
        node_bbox: BoundingBox,
    ) -> Result<cairo::Surface, RenderingError> {
        let surface = match filters {
            Filter::None => self.cr.get_target(),
            Filter::List(filter_list) => {
                if filter_list.is_applicable(node, acquired_nodes) {
                    // The target surface has multiple references.
                    // We need to copy it to a new surface to have a unique
                    // reference to be able to safely access the pixel data.
                    let child_surface = SharedImageSurface::copy_from_surface(
                        &cairo::ImageSurface::try_from(self.cr.get_target()).unwrap(),
                    )?;

                    let img_surface = filter_list
                        .iter()
                        .try_fold(
                            child_surface,
                            |surface, filter| -> Result<_, RenderingError> {
                                let FilterValue::URL(filter_uri) = filter;
                                self.run_filter(
                                    acquired_nodes,
                                    &filter_uri,
                                    node,
                                    values,
                                    surface,
                                    node_bbox,
                                )
                            },
                        )?
                        .into_image_surface()?;
                    // turn ImageSurface into a Surface
                    (*img_surface).clone()
                } else {
                    self.cr.get_target()
                }
            }
        };
        Ok(surface)
    }

    fn run_filter(
        &mut self,
        acquired_nodes: &mut AcquiredNodes,
        filter_uri: &Fragment,
        node: &Node,
        values: &ComputedValues,
        child_surface: SharedImageSurface,
        node_bbox: BoundingBox,
    ) -> Result<SharedImageSurface, RenderingError> {
        // TODO: since we check is_applicable before we get here, these checks are redundant
        // do we want to remove them and directly grab the filter node? or keep for future error
        // handling?
        match acquired_nodes.acquire(filter_uri) {
            Ok(acquired) => {
                let filter_node = acquired.get();

                match *filter_node.borrow_element() {
                    Element::Filter(_) => {
                        return filters::render(
                            &filter_node,
                            values,
                            child_surface,
                            acquired_nodes,
                            self,
                            self.get_transform(),
                            node_bbox,
                        );
                    }
                    _ => {
                        rsvg_log!(
                            "element {} will not be filtered since \"{}\" is not a filter",
                            node,
                            filter_uri,
                        );
                    }
                }
            }
            _ => {
                rsvg_log!(
                    "element {} will not be filtered since its filter \"{}\" was not found",
                    node,
                    filter_uri,
                );
            }
        }

        // Non-existing filters must act as null filters (an empty surface is returned).
        Ok(child_surface)
    }

    fn set_gradient(
        self: &mut DrawingCtx,
        gradient: &Gradient,
        _acquired_nodes: &mut AcquiredNodes,
        opacity: UnitInterval,
        values: &ComputedValues,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        let GradientUnits(units) = gradient.get_units();
        let transform = if let Ok(t) = bbox.rect_to_transform(units) {
            t
        } else {
            return Ok(false);
        };

        let params = self.push_coord_units(units);

        let g = match gradient.get_variant() {
            GradientVariant::Linear { x1, y1, x2, y2 } => {
                cairo::Gradient::clone(&cairo::LinearGradient::new(
                    x1.normalize(values, &params),
                    y1.normalize(values, &params),
                    x2.normalize(values, &params),
                    y2.normalize(values, &params),
                ))
            }

            GradientVariant::Radial {
                cx,
                cy,
                r,
                fx,
                fy,
                fr,
            } => {
                let n_cx = cx.normalize(values, &params);
                let n_cy = cy.normalize(values, &params);
                let n_r = r.normalize(values, &params);
                let n_fx = fx.normalize(values, &params);
                let n_fy = fy.normalize(values, &params);
                let n_fr = fr.normalize(values, &params);

                cairo::Gradient::clone(&cairo::RadialGradient::new(
                    n_fx, n_fy, n_fr, n_cx, n_cy, n_r,
                ))
            }
        };

        let transform = transform.pre_transform(&gradient.get_transform());
        if let Some(m) = transform.invert() {
            g.set_matrix(m.into())
        }

        g.set_extend(cairo::Extend::from(gradient.get_spread()));

        for stop in gradient.get_stops() {
            let UnitInterval(stop_offset) = stop.offset;
            let UnitInterval(o) = opacity;
            let UnitInterval(stop_opacity) = stop.opacity;

            g.add_color_stop_rgba(
                stop_offset,
                f64::from(stop.rgba.red_f32()),
                f64::from(stop.rgba.green_f32()),
                f64::from(stop.rgba.blue_f32()),
                f64::from(stop.rgba.alpha_f32()) * stop_opacity * o,
            );
        }

        let cr = self.cr.clone();
        cr.set_source(&g);

        Ok(true)
    }

    fn set_pattern(
        &mut self,
        pattern: &ResolvedPattern,
        acquired_nodes: &mut AcquiredNodes,
        opacity: UnitInterval,
        values: &ComputedValues,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        let node = if let Some(n) = pattern.node_with_children() {
            n
        } else {
            // This means we didn't find any children among the fallbacks,
            // so there is nothing to render.
            return Ok(false);
        };

        let units = pattern.get_units();
        let content_units = pattern.get_content_units();
        let pattern_transform = pattern.get_transform();

        let params = self.push_coord_units(units.0);

        let pattern_rect = pattern.get_rect(values, &params);

        // Work out the size of the rectangle so it takes into account the object bounding box
        let (bbwscale, bbhscale) = match units {
            PatternUnits(CoordUnits::ObjectBoundingBox) => bbox.rect.unwrap().size(),
            PatternUnits(CoordUnits::UserSpaceOnUse) => (1.0, 1.0),
        };

        let taffine = self.get_transform().pre_transform(&pattern_transform);

        let mut scwscale = (taffine.xx.powi(2) + taffine.xy.powi(2)).sqrt();
        let mut schscale = (taffine.yx.powi(2) + taffine.yy.powi(2)).sqrt();

        let scaled_width = pattern_rect.width() * bbwscale;
        let scaled_height = pattern_rect.height() * bbhscale;

        if approx_eq!(f64, scaled_width, 0.0) || approx_eq!(f64, scaled_height, 0.0) {
            return Ok(false);
        }

        let pw: i32 = (scaled_width * scwscale) as i32;
        let ph: i32 = (scaled_height * schscale) as i32;

        if pw < 1 || ph < 1 {
            return Ok(false);
        }

        scwscale = f64::from(pw) / scaled_width;
        schscale = f64::from(ph) / scaled_height;

        // Create the pattern coordinate system
        let mut affine = match units {
            PatternUnits(CoordUnits::ObjectBoundingBox) => {
                let bbrect = bbox.rect.unwrap();
                Transform::new_translate(
                    bbrect.x0 + pattern_rect.x0 * bbrect.width(),
                    bbrect.y0 + pattern_rect.y0 * bbrect.height(),
                )
            }

            PatternUnits(CoordUnits::UserSpaceOnUse) => {
                Transform::new_translate(pattern_rect.x0, pattern_rect.y0)
            }
        };

        // Apply the pattern transform
        affine = affine.post_transform(&pattern_transform);

        let mut caffine: Transform;

        // Create the pattern contents coordinate system
        let _params = if let Some(vbox) = pattern.get_vbox() {
            // If there is a vbox, use that
            let r = pattern
                .get_preserve_aspect_ratio()
                .compute(&vbox, &Rect::from_size(scaled_width, scaled_height));

            let sw = r.width() / vbox.width();
            let sh = r.height() / vbox.height();
            let x = r.x0 - vbox.x0 * sw;
            let y = r.y0 - vbox.y0 * sh;

            caffine = Transform::new_scale(sw, sh).pre_translate(x, y);

            self.push_view_box(vbox.width(), vbox.height())
        } else {
            let PatternContentUnits(content_units) = content_units;

            caffine = if content_units == CoordUnits::ObjectBoundingBox {
                // If coords are in terms of the bounding box, use them
                let (bbw, bbh) = bbox.rect.unwrap().size();
                Transform::new_scale(bbw, bbh)
            } else {
                Transform::identity()
            };

            self.push_coord_units(content_units)
        };

        if !scwscale.approx_eq_cairo(1.0) || !schscale.approx_eq_cairo(1.0) {
            caffine = caffine.post_scale(scwscale, schscale);
            affine = affine.pre_scale(1.0 / scwscale, 1.0 / schscale);
        }

        // Draw to another surface

        let cr_save = self.cr.clone();

        let surface = cr_save
            .get_target()
            .create_similar(cairo::Content::ColorAlpha, pw, ph)?;

        let cr_pattern = cairo::Context::new(&surface);

        self.set_cairo_context(&cr_pattern);

        // Set up transformations to be determined by the contents units
        cr_pattern.set_matrix(caffine.into());

        // Draw everything
        let res = self.with_alpha(opacity, &mut |dc| {
            let pattern_cascaded = CascadedValues::new_from_node(&node);
            let pattern_values = pattern_cascaded.get();
            dc.with_discrete_layer(
                &node,
                acquired_nodes,
                pattern_values,
                false,
                &mut |an, dc| node.draw_children(an, &pattern_cascaded, dc, false),
            )
        });

        // Return to the original coordinate system and rendering context
        self.set_cairo_context(&cr_save);

        // Set the final surface as a Cairo pattern into the Cairo context
        let pattern = cairo::SurfacePattern::create(&surface);

        if let Some(m) = affine.invert() {
            pattern.set_matrix(m.into())
        }
        pattern.set_extend(cairo::Extend::Repeat);
        pattern.set_filter(cairo::Filter::Best);
        cr_save.set_source(&pattern);

        res.map(|_| true)
    }

    fn set_color(
        &self,
        color: cssparser::Color,
        opacity: UnitInterval,
        current_color: cssparser::RGBA,
    ) -> Result<bool, RenderingError> {
        let rgba = match color {
            cssparser::Color::RGBA(rgba) => rgba,
            cssparser::Color::CurrentColor => current_color,
        };

        let UnitInterval(o) = opacity;
        self.cr.clone().set_source_rgba(
            f64::from(rgba.red_f32()),
            f64::from(rgba.green_f32()),
            f64::from(rgba.blue_f32()),
            f64::from(rgba.alpha_f32()) * o,
        );

        Ok(true)
    }

    fn set_source_paint_server(
        &mut self,
        acquired_nodes: &mut AcquiredNodes,
        paint_server: &PaintServer,
        opacity: UnitInterval,
        bbox: &BoundingBox,
        current_color: cssparser::RGBA,
        values: &ComputedValues,
    ) -> Result<bool, RenderingError> {
        let paint_source = paint_server.resolve(acquired_nodes)?;

        match paint_source {
            PaintSource::Gradient(g, c) => {
                if self.set_gradient(&g, acquired_nodes, opacity, values, bbox)? {
                    Ok(true)
                } else if let Some(c) = c {
                    self.set_color(c, opacity, current_color)
                } else {
                    Ok(false)
                }
            }
            PaintSource::Pattern(p, c) => {
                if self.set_pattern(&p, acquired_nodes, opacity, values, bbox)? {
                    Ok(true)
                } else if let Some(c) = c {
                    self.set_color(c, opacity, current_color)
                } else {
                    Ok(false)
                }
            }
            PaintSource::SolidColor(c) => self.set_color(c, opacity, current_color),
            PaintSource::None => Ok(false),
        }
    }

    /// Computes and returns a surface corresponding to the given paint server.
    pub fn get_paint_server_surface(
        &mut self,
        width: i32,
        height: i32,
        acquired_nodes: &mut AcquiredNodes,
        paint_server: &PaintServer,
        opacity: UnitInterval,
        bbox: &BoundingBox,
        current_color: cssparser::RGBA,
        values: &ComputedValues,
    ) -> Result<SharedImageSurface, cairo::Status> {
        let mut surface = ExclusiveImageSurface::new(width, height, SurfaceType::SRgb)?;

        surface.draw(&mut |cr| {
            let cr_save = self.cr.clone();
            self.set_cairo_context(&cr);

            // FIXME: we are ignoring any error
            let _ = self
                .set_source_paint_server(
                    acquired_nodes,
                    paint_server,
                    opacity,
                    bbox,
                    current_color,
                    values,
                )
                .map(|had_paint_server| {
                    if had_paint_server {
                        cr.paint();
                    }
                });

            self.set_cairo_context(&cr_save);

            Ok(())
        })?;

        surface.share()
    }

    fn setup_cr_for_stroke(&self, cr: &cairo::Context, values: &ComputedValues) {
        let params = self.get_view_params();

        cr.set_line_width(values.stroke_width().0.normalize(values, &params));
        cr.set_miter_limit(values.stroke_miterlimit().0);
        cr.set_line_cap(cairo::LineCap::from(values.stroke_line_cap()));
        cr.set_line_join(cairo::LineJoin::from(values.stroke_line_join()));

        if let StrokeDasharray(Dasharray::Array(ref dashes)) = values.stroke_dasharray() {
            let normalized_dashes: Vec<f64> = dashes
                .iter()
                .map(|l| l.normalize(values, &params))
                .collect();

            let total_length = normalized_dashes.iter().fold(0.0, |acc, &len| acc + len);

            if total_length > 0.0 {
                let offset = values.stroke_dashoffset().0.normalize(values, &params);
                cr.set_dash(&normalized_dashes, offset);
            } else {
                cr.set_dash(&[], 0.0);
            }
        }
    }

    fn stroke(
        &mut self,
        cr: &cairo::Context,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        bbox: &BoundingBox,
        current_color: cssparser::RGBA,
    ) -> Result<(), RenderingError> {
        self.set_source_paint_server(
            acquired_nodes,
            &values.stroke().0,
            values.stroke_opacity().0,
            bbox,
            current_color,
            values,
        )
        .map(|had_paint_server| {
            if had_paint_server {
                cr.stroke_preserve();
            }
        })?;

        Ok(())
    }

    fn fill(
        &mut self,
        cr: &cairo::Context,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        bbox: &BoundingBox,
        current_color: cssparser::RGBA,
    ) -> Result<(), RenderingError> {
        self.set_source_paint_server(
            acquired_nodes,
            &values.fill().0,
            values.fill_opacity().0,
            bbox,
            current_color,
            values,
        )
        .map(|had_paint_server| {
            if had_paint_server {
                cr.fill_preserve();
            }
        })?;

        Ok(())
    }

    pub fn draw_path(
        &mut self,
        path: &Path,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        markers: Markers,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if path.is_empty() {
            return Ok(self.empty_bbox());
        }

        self.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
            let cr = dc.cr.clone();
            let mut path_helper = PathHelper::new(&cr, path, values);

            if clipping {
                cr.set_fill_rule(cairo::FillRule::from(values.clip_rule()));
                path_helper.set()?;
                return Ok(dc.empty_bbox());
            }

            let current_color = values.color().0;

            cr.set_antialias(cairo::Antialias::from(values.shape_rendering()));
            dc.setup_cr_for_stroke(&cr, values);

            cr.set_fill_rule(cairo::FillRule::from(values.fill_rule()));

            let mut bounding_box: Option<BoundingBox> = None;
            path_helper.unset();

            let params = dc.get_view_params();

            for &target in &values.paint_order().targets {
                // fill and stroke operations will preserve the path.
                // markers operation will clear the path.
                match target {
                    PaintTarget::Fill | PaintTarget::Stroke => {
                        path_helper.set()?;
                        let bbox = bounding_box.get_or_insert_with(|| {
                            compute_stroke_and_fill_box(&cr, &values, &params)
                        });

                        if target == PaintTarget::Stroke {
                            dc.stroke(&cr, an, values, &bbox, current_color)?;
                        } else {
                            dc.fill(&cr, an, values, &bbox, current_color)?;
                        }
                    }
                    PaintTarget::Markers if markers == Markers::Yes => {
                        path_helper.unset();
                        marker::render_markers_for_path(path, dc, an, values, clipping)?;
                    }
                    _ => {}
                }
            }

            path_helper.unset();
            Ok(bounding_box.unwrap())
        })
    }

    pub fn draw_image(
        &mut self,
        surface: &SharedImageSurface,
        rect: Rect,
        aspect: AspectRatio,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let image_width = surface.width();
        let image_height = surface.height();
        if clipping || rect.is_empty() || image_width == 0 || image_height == 0 {
            return Ok(self.empty_bbox());
        }

        let image_width = f64::from(image_width);
        let image_height = f64::from(image_height);
        let vbox = ViewBox::from(Rect::from_size(image_width, image_height));

        let clip_mode = if !values.is_overflow() && aspect.is_slice() {
            Some(ClipMode::ClipToViewport)
        } else {
            None
        };

        self.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |_an, dc| {
            dc.with_saved_cr(&mut |dc| {
                if let Some(_params) = dc.push_new_viewport(Some(vbox), rect, aspect, clip_mode) {
                    let cr = dc.cr.clone();

                    // We need to set extend appropriately, so can't use cr.set_source_surface().
                    //
                    // If extend is left at its default value (None), then bilinear scaling uses
                    // transparency outside of the image producing incorrect results.
                    // For example, in svg1.1/filters-blend-01-b.svgthere's a completely
                    // opaque 100×1 image of a gradient scaled to 100×98 which ends up
                    // transparent almost everywhere without this fix (which it shouldn't).
                    let ptn = surface.to_cairo_pattern();
                    ptn.set_extend(cairo::Extend::Pad);
                    cr.set_source(&ptn);

                    // Clip is needed due to extend being set to pad.
                    clip_to_rectangle(&cr, &Rect::from_size(image_width, image_height));

                    cr.paint();
                }

                // The bounding box for <image> is decided by the values of x, y, w, h
                // and not by the final computed image bounds.
                Ok(dc.empty_bbox().with_rect(rect))
            })
        })
    }

    pub fn draw_text(
        &mut self,
        layout: &pango::Layout,
        x: f64,
        y: f64,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let transform = self.get_transform();

        let gravity = layout.get_context().unwrap().get_gravity();

        let bbox = compute_text_box(layout, x, y, transform, gravity);
        if bbox.is_none() {
            return Ok(self.empty_bbox());
        }

        let mut bbox = if clipping {
            self.empty_bbox()
        } else {
            bbox.unwrap()
        };

        self.with_saved_cr(&mut |dc| {
            let cr = dc.cr.clone();

            cr.set_antialias(cairo::Antialias::from(values.text_rendering()));
            dc.setup_cr_for_stroke(&cr, &values);
            cr.move_to(x, y);

            let rotation = gravity.to_rotation();
            if !rotation.approx_eq_cairo(0.0) {
                cr.rotate(-rotation);
            }

            let current_color = values.color().0;

            let res = if !clipping {
                dc.set_source_paint_server(
                    acquired_nodes,
                    &values.fill().0,
                    values.fill_opacity().0,
                    &bbox,
                    current_color,
                    values,
                )
                .map(|had_paint_server| {
                    if had_paint_server {
                        pangocairo::functions::update_layout(&cr, &layout);
                        pangocairo::functions::show_layout(&cr, &layout);
                    };
                })
            } else {
                Ok(())
            };

            if res.is_ok() {
                let mut need_layout_path = clipping;

                let res = if !clipping {
                    dc.set_source_paint_server(
                        acquired_nodes,
                        &values.stroke().0,
                        values.stroke_opacity().0,
                        &bbox,
                        current_color,
                        values,
                    )
                    .map(|had_paint_server| {
                        if had_paint_server {
                            need_layout_path = true;
                        }
                    })
                } else {
                    Ok(())
                };

                if res.is_ok() && need_layout_path {
                    pangocairo::functions::update_layout(&cr, &layout);
                    pangocairo::functions::layout_path(&cr, &layout);

                    if !clipping {
                        let (x0, y0, x1, y1) = cr.stroke_extents();
                        let r = Rect::new(x0, y0, x1, y1);
                        let ib = BoundingBox::new()
                            .with_transform(transform)
                            .with_ink_rect(r);
                        cr.stroke();
                        bbox.insert(&ib);
                    }
                }
            }

            res.map(|_: ()| bbox)
        })
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

    pub fn draw_node_to_surface(
        &mut self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        affine: Transform,
        width: i32,
        height: i32,
    ) -> Result<SharedImageSurface, RenderingError> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;

        let save_initial_viewport = self.initial_viewport;
        let save_cr = self.cr.clone();

        {
            let cr = cairo::Context::new(&surface);
            cr.set_matrix(affine.into());

            self.cr = cr;
            self.initial_viewport = Viewport {
                transform: affine,
                vbox: ViewBox::from(Rect::from_size(f64::from(width), f64::from(height))),
            };

            let _ = self.draw_node_from_stack(node, acquired_nodes, cascaded, false)?;
        }

        self.cr = save_cr;
        self.initial_viewport = save_initial_viewport;

        Ok(SharedImageSurface::wrap(surface, SurfaceType::SRgb)?)
    }

    pub fn draw_node_from_stack(
        &mut self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
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
            node.draw(acquired_nodes, cascaded, self, clipping)
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
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        link: Option<&Fragment>,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        // <use> is an element that is used directly, unlike
        // <pattern>, which is used through a fill="url(#...)"
        // reference.  However, <use> will always reference another
        // element, potentially itself or an ancestor of itself (or
        // another <use> which references the first one, etc.).  So,
        // we acquire the <use> element itself so that circular
        // references can be caught.
        let _self_acquired = acquired_nodes.acquire_ref(node).map_err(|e| {
            if let AcquireError::CircularReference(_) = e {
                rsvg_log!("circular reference in element {}", node);
                RenderingError::CircularReference
            } else {
                unreachable!();
            }
        })?;

        if link.is_none() {
            return Ok(self.empty_bbox());
        }

        let acquired = match acquired_nodes.acquire(link.unwrap()) {
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
        let use_rect = borrow_element_as!(node, Use).get_rect(values, &params);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if use_rect.is_empty() {
            return Ok(self.empty_bbox());
        }

        let child = acquired.get();

        // if it is a symbol
        if child.is_element() {
            let elt = child.borrow_element();

            if let Element::Symbol(ref symbol) = *elt {
                let clip_mode = if !values.is_overflow()
                    || (values.overflow() == Overflow::Visible
                        && elt.get_specified_values().is_overflow())
                {
                    Some(ClipMode::ClipToVbox)
                } else {
                    None
                };

                return self.with_discrete_layer(
                    node,
                    acquired_nodes,
                    values,
                    clipping,
                    &mut |an, dc| {
                        let _params = dc.push_new_viewport(
                            symbol.get_viewbox(),
                            use_rect,
                            symbol.get_preserve_aspect_ratio(),
                            clip_mode,
                        );

                        child.draw_children(
                            an,
                            &CascadedValues::new_from_values(&child, values),
                            dc,
                            clipping,
                        )
                    },
                );
            }
        };

        // all other nodes
        let cr = self.cr.clone();
        cr.translate(use_rect.x0, use_rect.y0);

        self.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
            dc.draw_node_from_stack(
                &child,
                an,
                &CascadedValues::new_from_values(&child, values),
                clipping,
            )
        })
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

// Returns (clip_in_user_space, clip_in_object_space), both Option<Node>
fn get_clip_in_user_and_object_space(
    acquired_nodes: &mut AcquiredNodes,
    clip_uri: Option<&Fragment>,
) -> (Option<Node>, Option<Node>) {
    clip_uri
        .and_then(|fragment| {
            acquired_nodes
                .acquire(fragment)
                .ok()
                .filter(|a| is_element_of_type!(*a.get(), ClipPath))
        })
        .map(|acquired| {
            let clip_node = acquired.get().clone();

            let units = borrow_element_as!(clip_node, ClipPath).get_units();

            match units {
                CoordUnits::UserSpaceOnUse => (Some(clip_node), None),
                CoordUnits::ObjectBoundingBox => (None, Some(clip_node)),
            }
        })
        .unwrap_or((None, None))
}

fn compute_stroke_and_fill_box(
    cr: &cairo::Context,
    values: &ComputedValues,
    params: &ViewParams,
) -> BoundingBox {
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
    //
    // When presented with a line width of 0, Cairo returns a
    // stroke_extents rectangle of (0, 0, 0, 0).  This would cause the
    // bbox to include a lone point at the origin, which is wrong, as a
    // stroke of zero width should not be painted, per
    // https://www.w3.org/TR/SVG2/painting.html#StrokeWidth
    //
    // So, see if the stroke width is 0 and just not include the stroke in the
    // bounding box if so.

    let stroke_width = values.stroke_width().0.normalize(values, &params);

    if !stroke_width.approx_eq_cairo(0.0) && values.stroke().0 != PaintServer::None {
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

fn compute_text_box(
    layout: &pango::Layout,
    x: f64,
    y: f64,
    transform: Transform,
    gravity: pango::Gravity,
) -> Option<BoundingBox> {
    #![allow(clippy::many_single_char_names)]

    let (ink, _) = layout.get_extents();
    if ink.width == 0 || ink.height == 0 {
        return None;
    }

    let ink_x = f64::from(ink.x);
    let ink_y = f64::from(ink.y);
    let ink_width = f64::from(ink.width);
    let ink_height = f64::from(ink.height);
    let pango_scale = f64::from(pango::SCALE);

    let (x, y, w, h) = if gravity_is_vertical(gravity) {
        (
            x + (ink_x - ink_height) / pango_scale,
            y + ink_y / pango_scale,
            ink_height / pango_scale,
            ink_width / pango_scale,
        )
    } else {
        (
            x + ink_x / pango_scale,
            y + ink_y / pango_scale,
            ink_width / pango_scale,
            ink_height / pango_scale,
        )
    };

    let r = Rect::new(x, y, x + w, y + h);
    let bbox = BoundingBox::new()
        .with_transform(transform)
        .with_rect(r)
        .with_ink_rect(r);

    Some(bbox)
}

// FIXME: should the pango crate provide this like PANGO_GRAVITY_IS_VERTICAL() ?
fn gravity_is_vertical(gravity: pango::Gravity) -> bool {
    match gravity {
        pango::Gravity::East | pango::Gravity::West => true,
        _ => false,
    }
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

fn clip_to_rectangle(cr: &cairo::Context, r: &Rect) {
    cr.rectangle(r.x0, r.y0, r.width(), r.height());
    cr.clip();
}

impl From<SpreadMethod> for cairo::Extend {
    fn from(s: SpreadMethod) -> cairo::Extend {
        match s {
            SpreadMethod::Pad => cairo::Extend::Pad,
            SpreadMethod::Reflect => cairo::Extend::Reflect,
            SpreadMethod::Repeat => cairo::Extend::Repeat,
        }
    }
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

impl From<MixBlendMode> for cairo::Operator {
    fn from(m: MixBlendMode) -> cairo::Operator {
        use cairo::Operator;

        match m {
            MixBlendMode::Normal => Operator::Over,
            MixBlendMode::Multiply => Operator::Multiply,
            MixBlendMode::Screen => Operator::Screen,
            MixBlendMode::Overlay => Operator::Overlay,
            MixBlendMode::Darken => Operator::Darken,
            MixBlendMode::Lighten => Operator::Lighten,
            MixBlendMode::ColorDodge => Operator::ColorDodge,
            MixBlendMode::ColorBurn => Operator::ColorBurn,
            MixBlendMode::HardLight => Operator::HardLight,
            MixBlendMode::SoftLight => Operator::SoftLight,
            MixBlendMode::Difference => Operator::Difference,
            MixBlendMode::Exclusion => Operator::Exclusion,
            MixBlendMode::Hue => Operator::HslHue,
            MixBlendMode::Saturation => Operator::HslSaturation,
            MixBlendMode::Color => Operator::HslColor,
            MixBlendMode::Luminosity => Operator::HslLuminosity,
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

impl From<TextRendering> for cairo::Antialias {
    fn from(tr: TextRendering) -> cairo::Antialias {
        match tr {
            TextRendering::Auto
            | TextRendering::OptimizeLegibility
            | TextRendering::GeometricPrecision => cairo::Antialias::Default,
            TextRendering::OptimizeSpeed => cairo::Antialias::None,
        }
    }
}

impl From<&DrawingCtx> for pango::Context {
    fn from(draw_ctx: &DrawingCtx) -> pango::Context {
        let cr = draw_ctx.cr.clone();
        let font_map = pangocairo::FontMap::get_default().unwrap();
        let context = font_map.create_context().unwrap();
        pangocairo::functions::update_context(&cr, &context);

        // Pango says this about pango_cairo_context_set_resolution():
        //
        //     Sets the resolution for the context. This is a scale factor between
        //     points specified in a #PangoFontDescription and Cairo units. The
        //     default value is 96, meaning that a 10 point font will be 13
        //     units high. (10 * 96. / 72. = 13.3).
        //
        // I.e. Pango font sizes in a PangoFontDescription are in *points*, not pixels.
        // However, we are normalizing everything to userspace units, which amount to
        // pixels.  So, we will use 72.0 here to make Pango not apply any further scaling
        // to the size values we give it.
        //
        // An alternative would be to divide our font sizes by (dpi_y / 72) to effectively
        // cancel out Pango's scaling, but it's probably better to deal with Pango-isms
        // right here, instead of spreading them out through our Length normalization
        // code.
        pangocairo::functions::context_set_resolution(&context, 72.0);

        if draw_ctx.testing {
            let mut options = cairo::FontOptions::new();

            options.set_antialias(cairo::Antialias::Gray);
            options.set_hint_style(cairo::HintStyle::Full);
            options.set_hint_metrics(cairo::HintMetrics::On);

            pangocairo::functions::context_set_font_options(&context, Some(&options));
        }

        context
    }
}
