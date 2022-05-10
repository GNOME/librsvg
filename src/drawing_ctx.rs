//! The main context structure which drives the drawing process.

use cssparser::RGBA;
use float_cmp::approx_eq;
use glib::translate::*;
use once_cell::sync::Lazy;
use pango::ffi::PangoMatrix;
use pango::prelude::FontMapExt;
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::f64::consts::*;
use std::rc::{Rc, Weak};

use crate::accept_language::UserLanguage;
use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNodes, NodeId};
use crate::dpi::Dpi;
use crate::element::Element;
use crate::error::{AcquireError, ImplementationLimit, RenderingError};
use crate::filter::FilterValueList;
use crate::filters::{self, FilterSpec};
use crate::float_eq_cairo::ApproxEqCairo;
use crate::gradient::{GradientVariant, SpreadMethod, UserSpaceGradient};
use crate::layout::{Image, Shape, StackingContext, Stroke, Text, TextSpan};
use crate::length::*;
use crate::marker;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::paint_server::{PaintSource, UserSpacePaintSource};
use crate::path_builder::*;
use crate::pattern::UserSpacePattern;
use crate::properties::{
    ClipRule, ComputedValues, FillRule, Filter, Isolation, MaskType, MixBlendMode, Opacity,
    Overflow, PaintTarget, ShapeRendering, StrokeLinecap, StrokeLinejoin, TextRendering,
};
use crate::rect::{IRect, Rect};
use crate::surface_utils::{
    shared_surface::ExclusiveImageSurface, shared_surface::SharedImageSurface,
    shared_surface::SurfaceType,
};
use crate::transform::Transform;
use crate::unit_interval::UnitInterval;
use crate::viewbox::ViewBox;

/// Holds values that are required to normalize `CssLength` values to a current viewport.
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

    pub fn with_units(&self, units: CoordUnits) -> ViewParams {
        match units {
            CoordUnits::ObjectBoundingBox => ViewParams {
                dpi: self.dpi,
                vbox: ViewBox::from(Rect::from_size(1.0, 1.0)),
                viewport_stack: None,
            },

            CoordUnits::UserSpaceOnUse => ViewParams {
                dpi: self.dpi,
                vbox: self.vbox,
                viewport_stack: None,
            },
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

/// Opaque font options for a DrawingCtx.
///
/// This is used for DrawingCtx::create_pango_context.
pub struct FontOptions {
    options: cairo::FontOptions,
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
    transform: Transform,
    path: &'a Path,
    is_square_linecap: bool,
    has_path: Option<bool>,
}

impl<'a> PathHelper<'a> {
    pub fn new(
        cr: &'a cairo::Context,
        transform: Transform,
        path: &'a Path,
        linecap: StrokeLinecap,
    ) -> Self {
        PathHelper {
            cr,
            transform,
            path,
            is_square_linecap: linecap == StrokeLinecap::Square,
            has_path: None,
        }
    }

    pub fn set(&mut self) -> Result<(), RenderingError> {
        match self.has_path {
            Some(false) | None => {
                self.has_path = Some(true);
                self.cr.set_matrix(self.transform.into());
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

    cr_stack: Rc<RefCell<Vec<cairo::Context>>>,
    cr: cairo::Context,

    user_language: UserLanguage,

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
    user_language: &UserLanguage,
    dpi: Dpi,
    measuring: bool,
    testing: bool,
    acquired_nodes: &mut AcquiredNodes<'_>,
) -> Result<BoundingBox, RenderingError> {
    let (drawsub_stack, node) = match mode {
        DrawingMode::LimitToStack { node, root } => (node.ancestors().collect(), root),

        DrawingMode::OnlyNode(node) => (Vec::new(), node),
    };

    let cascaded = CascadedValues::new_from_node(&node);

    // Preserve the user's transform and use it for the outermost bounding box.  All bounds/extents
    // will be converted to this transform in the end.
    let user_transform = Transform::from(cr.matrix());
    let mut user_bbox = BoundingBox::new().with_transform(user_transform);

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
    let transform = user_transform.pre_translate(viewport.x0, viewport.y0);
    cr.set_matrix(transform.into());

    // Per the spec, so the viewport has (0, 0) as upper-left.
    let viewport = viewport.translate((-viewport.x0, -viewport.y0));

    let mut draw_ctx = DrawingCtx::new(
        cr,
        transform,
        viewport,
        user_language.clone(),
        dpi,
        measuring,
        testing,
        drawsub_stack,
    );

    let content_bbox = draw_ctx.draw_node_from_stack(&node, acquired_nodes, &cascaded, false)?;

    user_bbox.insert(&content_bbox);

    Ok(user_bbox)
}

pub fn with_saved_cr<O, F>(cr: &cairo::Context, f: F) -> Result<O, RenderingError>
where
    F: FnOnce() -> Result<O, RenderingError>,
{
    cr.save()?;
    match f() {
        Ok(o) => {
            cr.restore()?;
            Ok(o)
        }

        Err(e) => Err(e),
    }
}

impl Drop for DrawingCtx {
    fn drop(&mut self) {
        self.cr_stack.borrow_mut().pop();
    }
}

const CAIRO_TAG_LINK: &str = "Link";

impl DrawingCtx {
    fn new(
        cr: &cairo::Context,
        transform: Transform,
        viewport: Rect,
        user_language: UserLanguage,
        dpi: Dpi,
        measuring: bool,
        testing: bool,
        drawsub_stack: Vec<Node>,
    ) -> DrawingCtx {
        let vbox = ViewBox::from(viewport);
        let initial_viewport = Viewport { transform, vbox };

        let viewport_stack = vec![initial_viewport];

        DrawingCtx {
            initial_viewport,
            dpi,
            cr_stack: Rc::new(RefCell::new(Vec::new())),
            cr: cr.clone(),
            user_language,
            viewport_stack: Rc::new(RefCell::new(viewport_stack)),
            drawsub_stack,
            measuring,
            testing,
        }
    }

    /// Copies a `DrawingCtx` for temporary use on a Cairo surface.
    ///
    /// `DrawingCtx` maintains state using during the drawing process, and sometimes we
    /// would like to use that same state but on a different Cairo surface and context
    /// than the ones being used on `self`.  This function copies the `self` state into a
    /// new `DrawingCtx`, and ties the copied one to the supplied `cr`.
    fn nested(&self, cr: cairo::Context) -> DrawingCtx {
        let cr_stack = self.cr_stack.clone();

        cr_stack.borrow_mut().push(self.cr.clone());

        DrawingCtx {
            initial_viewport: self.initial_viewport,
            dpi: self.dpi,
            cr_stack,
            cr,
            user_language: self.user_language.clone(),
            viewport_stack: self.viewport_stack.clone(),
            drawsub_stack: self.drawsub_stack.clone(),
            measuring: self.measuring,
            testing: self.testing,
        }
    }

    pub fn user_language(&self) -> &UserLanguage {
        &self.user_language
    }

    pub fn toplevel_viewport(&self) -> Rect {
        *self.initial_viewport.vbox
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring
    }

    fn get_transform(&self) -> Transform {
        Transform::from(self.cr.matrix())
    }

    pub fn empty_bbox(&self) -> BoundingBox {
        BoundingBox::new().with_transform(self.get_transform())
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

    // Same as `push_coord_units` but doesn't leave the coordinate space pushed
    pub fn get_view_params_for_units(&self, units: CoordUnits) -> ViewParams {
        match units {
            CoordUnits::ObjectBoundingBox => ViewParams {
                dpi: self.dpi,
                vbox: ViewBox::from(Rect::from_size(1.0, 1.0)),
                viewport_stack: None,
            },

            CoordUnits::UserSpaceOnUse => ViewParams {
                dpi: self.dpi,
                vbox: self.get_top_viewport().vbox,
                viewport_stack: None,
            },
        }
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
    /// With the returned `ViewParams`, plus a `ComputedValues`, you can create a
    /// `NormalizeParams` that can be used with calls to `CssLength.to_user()` that
    /// correspond to this viewport.
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
    /// inside `with_saved_cr` or `draw_ctx.with_discrete_layer`.
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
            .unwrap_or_else(|_e| {
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
        acquired_nodes: &mut AcquiredNodes<'_>,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        if clip_node.is_none() {
            return Ok(());
        }

        let node = clip_node.as_ref().unwrap();
        let units = borrow_element_as!(node, ClipPath).get_units();

        if let Ok(transform) = bbox.rect_to_transform(units) {
            let cascaded = CascadedValues::new_from_node(node);
            let values = cascaded.get();

            let node_transform = values.transform().post_transform(&transform);

            let orig_transform = self.get_transform();
            self.cr.transform(node_transform.into());

            for child in node.children().filter(|c| {
                c.is_element() && element_can_be_used_inside_clip_path(&c.borrow_element())
            }) {
                child.draw(
                    acquired_nodes,
                    &CascadedValues::clone_with_node(&cascaded, &child),
                    self,
                    true,
                )?;
            }

            self.cr.clip();

            self.cr.set_matrix(orig_transform.into());
        }

        Ok(())
    }

    fn generate_cairo_mask(
        &mut self,
        mask_node: &Node,
        transform: Transform,
        bbox: &BoundingBox,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<Option<cairo::ImageSurface>, RenderingError> {
        if bbox.rect.is_none() {
            // The node being masked is empty / doesn't have a
            // bounding box, so there's nothing to mask!
            return Ok(None);
        }

        let _mask_acquired = match acquired_nodes.acquire_ref(mask_node) {
            Ok(n) => n,

            Err(AcquireError::CircularReference(_)) => {
                rsvg_log!("circular reference in element {}", mask_node);
                return Ok(None);
            }

            _ => unreachable!(),
        };

        let mask = borrow_element_as!(mask_node, Mask);

        let bbox_rect = bbox.rect.as_ref().unwrap();

        let cascaded = CascadedValues::new_from_node(mask_node);
        let values = cascaded.get();

        let mask_units = mask.get_units();

        let mask_rect = {
            let params = NormalizeParams::new(values, &self.get_view_params_for_units(mask_units));
            mask.get_rect(&params)
        };

        let mask_element = mask_node.borrow_element();

        let mask_transform = values.transform().post_transform(&transform);

        let mask_content_surface = self.create_surface_for_toplevel_viewport()?;

        // Use a scope because mask_cr needs to release the
        // reference to the surface before we access the pixels
        {
            let mask_cr = cairo::Context::new(&mask_content_surface)?;
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

            // TODO: this is the last place where push_coord_units() is called.  The call to
            // draw_children below assumes that the new coordinate system is in place.  Can we
            // pass the ViewParams to with_discrete_layer / Node::draw instead of having them
            // assume the viewport from the DrawingCtx?
            let _params = self.push_coord_units(mask.get_content_units());

            let mut mask_draw_ctx = self.nested(mask_cr);

            let stacking_ctx =
                StackingContext::new(acquired_nodes, &mask_element, Transform::identity(), values);

            let res = mask_draw_ctx.with_discrete_layer(
                &stacking_ctx,
                acquired_nodes,
                values,
                false,
                None,
                &mut |an, dc, _transform| mask_node.draw_children(an, &cascaded, dc, false),
            );

            res?;
        }

        let tmp = SharedImageSurface::wrap(mask_content_surface, SurfaceType::SRgb)?;

        let mask_result = match values.mask_type() {
            MaskType::Luminance => tmp.to_luminance_mask()?,
            MaskType::Alpha => tmp.extract_alpha(IRect::from_size(tmp.width(), tmp.height()))?,
        };

        let mask = mask_result.into_image_surface()?;

        Ok(Some(mask))
    }

    pub fn with_discrete_layer(
        &mut self,
        stacking_ctx: &StackingContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        clipping: bool,
        clip_rect: Option<Rect>,
        draw_fn: &mut dyn FnMut(
            &mut AcquiredNodes<'_>,
            &mut DrawingCtx,
            &Transform,
        ) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        if !stacking_ctx.transform.is_invertible() {
            // https://www.w3.org/TR/css-transforms-1/#transform-function-lists
            //
            // "If a transform function causes the current transformation matrix of an
            // object to be non-invertible, the object and its content do not get
            // displayed."
            return Ok(self.empty_bbox());
        }

        let orig_transform = self.get_transform();
        self.cr.transform(stacking_ctx.transform.into());

        let res = if clipping {
            let current_transform = self.get_transform();
            draw_fn(acquired_nodes, self, &current_transform)
        } else {
            with_saved_cr(&self.cr.clone(), || {
                if let Some(ref link_target) = stacking_ctx.link_target {
                    self.link_tag_begin(link_target);
                }

                let Opacity(UnitInterval(opacity)) = stacking_ctx.opacity;

                let affine_at_start = self.get_transform();

                if let Some(rect) = clip_rect {
                    clip_to_rectangle(&self.cr, &rect);
                }

                // Here we are clipping in user space, so the bbox doesn't matter
                self.clip_to_node(
                    &stacking_ctx.clip_in_user_space,
                    acquired_nodes,
                    &self.empty_bbox(),
                )?;

                let should_isolate = match stacking_ctx.isolation {
                    Isolation::Auto => {
                        let is_opaque = approx_eq!(f64, opacity, 1.0);
                        !(is_opaque
                            && stacking_ctx.filter == Filter::None
                            && stacking_ctx.mask.is_none()
                            && stacking_ctx.mix_blend_mode == MixBlendMode::Normal
                            && stacking_ctx.clip_in_object_space.is_none())
                    }
                    Isolation::Isolate => true,
                };

                let res = if should_isolate {
                    // Compute our assortment of affines

                    let affines = CompositingAffines::new(
                        affine_at_start,
                        self.initial_viewport.transform,
                        self.cr_stack.borrow().len(),
                    );

                    // Create temporary surface and its cr

                    let cr = match stacking_ctx.filter {
                        Filter::None => cairo::Context::new(
                            &self
                                .create_similar_surface_for_toplevel_viewport(&self.cr.target())?,
                        )?,
                        Filter::List(_) => {
                            cairo::Context::new(&*self.create_surface_for_toplevel_viewport()?)?
                        }
                    };

                    cr.set_matrix(affines.for_temporary_surface.into());

                    let (source_surface, mut res, bbox) = {
                        let mut temporary_draw_ctx = self.nested(cr);

                        // Draw!

                        let temporary_transform = temporary_draw_ctx.get_transform();
                        let res = draw_fn(
                            acquired_nodes,
                            &mut temporary_draw_ctx,
                            &temporary_transform,
                        );

                        let bbox = if let Ok(ref bbox) = res {
                            *bbox
                        } else {
                            BoundingBox::new().with_transform(affines.for_temporary_surface)
                        };

                        if let Filter::List(ref filter_list) = stacking_ctx.filter {
                            let surface_to_filter = SharedImageSurface::copy_from_surface(
                                &cairo::ImageSurface::try_from(temporary_draw_ctx.cr.target())
                                    .unwrap(),
                            )?;

                            let current_color = values.color().0;

                            let params = temporary_draw_ctx.get_view_params();

                            // TODO: the stroke/fill paint are already resolved for shapes.  Outside of shapes,
                            // they are also needed for filters in all elements.  Maybe we should make them part
                            // of the StackingContext instead of Shape?
                            let stroke_paint_source = Rc::new(
                                values
                                    .stroke()
                                    .0
                                    .resolve(
                                        acquired_nodes,
                                        values.stroke_opacity().0,
                                        current_color,
                                        None,
                                        None,
                                    )
                                    .to_user_space(&bbox, &params, values),
                            );

                            let fill_paint_source = Rc::new(
                                values
                                    .fill()
                                    .0
                                    .resolve(
                                        acquired_nodes,
                                        values.fill_opacity().0,
                                        current_color,
                                        None,
                                        None,
                                    )
                                    .to_user_space(&bbox, &params, values),
                            );

                            // Filter functions (like "blend()", not the <filter> element) require
                            // being resolved in userSpaceonUse units, since that is the default
                            // for primitive_units.  So, get the corresponding NormalizeParams
                            // here and pass them down.
                            let user_space_params = NormalizeParams::new(
                                values,
                                &params.with_units(CoordUnits::UserSpaceOnUse),
                            );

                            let filtered_surface = temporary_draw_ctx
                                .run_filters(
                                    surface_to_filter,
                                    filter_list,
                                    acquired_nodes,
                                    &stacking_ctx.element_name,
                                    &user_space_params,
                                    stroke_paint_source,
                                    fill_paint_source,
                                    current_color,
                                    bbox,
                                )?
                                .into_image_surface()?;

                            let generic_surface: &cairo::Surface = &filtered_surface; // deref to Surface

                            (generic_surface.clone(), res, bbox)
                        } else {
                            (temporary_draw_ctx.cr.target(), res, bbox)
                        }
                    };

                    // Set temporary surface as source

                    self.cr.set_matrix(affines.compositing.into());
                    self.cr.set_source_surface(&source_surface, 0.0, 0.0)?;

                    // Clip

                    self.cr.set_matrix(affines.outside_temporary_surface.into());
                    self.clip_to_node(&stacking_ctx.clip_in_object_space, acquired_nodes, &bbox)?;

                    // Mask

                    if let Some(ref mask_node) = stacking_ctx.mask {
                        res = res.and_then(|bbox| {
                            self.generate_cairo_mask(
                                mask_node,
                                affines.for_temporary_surface,
                                &bbox,
                                acquired_nodes,
                            )
                            .and_then(|mask_surf| {
                                if let Some(surf) = mask_surf {
                                    self.cr.push_group();

                                    self.cr.set_matrix(affines.compositing.into());
                                    self.cr.mask_surface(&surf, 0.0, 0.0)?;

                                    Ok(self.cr.pop_group_to_source()?)
                                } else {
                                    Ok(())
                                }
                            })
                            .map(|_: ()| bbox)
                        });
                    }

                    {
                        // Composite the temporary surface

                        self.cr.set_matrix(affines.compositing.into());
                        self.cr.set_operator(stacking_ctx.mix_blend_mode.into());

                        if opacity < 1.0 {
                            self.cr.paint_with_alpha(opacity)?;
                        } else {
                            self.cr.paint()?;
                        }
                    }

                    self.cr.set_matrix(affine_at_start.into());
                    res
                } else {
                    let current_transform = self.get_transform();
                    draw_fn(acquired_nodes, self, &current_transform)
                };

                if stacking_ctx.link_target.is_some() {
                    self.link_tag_end();
                }

                res
            })
        };

        self.cr.set_matrix(orig_transform.into());
        res
    }

    /// Run the drawing function with the specified opacity
    fn with_alpha(
        &mut self,
        opacity: UnitInterval,
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        let res;
        let UnitInterval(o) = opacity;
        if o < 1.0 {
            self.cr.push_group();
            res = draw_fn(self);
            self.cr.pop_group_to_source()?;
            self.cr.paint_with_alpha(o)?;
        } else {
            res = draw_fn(self);
        }

        res
    }

    /// Start a Cairo tag for PDF links
    fn link_tag_begin(&mut self, link_target: &str) {
        let attributes = format!("uri='{}'", escape_link_target(link_target));

        let cr = self.cr.clone();
        cr.tag_begin(CAIRO_TAG_LINK, &attributes);
    }

    /// End a Cairo tag for PDF links
    fn link_tag_end(&mut self) {
        self.cr.tag_end(CAIRO_TAG_LINK);
    }

    fn run_filters(
        &mut self,
        surface_to_filter: SharedImageSurface,
        filter_list: &FilterValueList,
        acquired_nodes: &mut AcquiredNodes<'_>,
        node_name: &str,
        user_space_params: &NormalizeParams,
        stroke_paint_source: Rc<UserSpacePaintSource>,
        fill_paint_source: Rc<UserSpacePaintSource>,
        current_color: RGBA,
        node_bbox: BoundingBox,
    ) -> Result<SharedImageSurface, RenderingError> {
        let surface = if let Ok(specs) = filter_list
            .iter()
            .map(|filter_value| {
                filter_value.to_filter_spec(
                    acquired_nodes,
                    user_space_params,
                    current_color,
                    self,
                    node_name,
                )
            })
            .collect::<Result<Vec<FilterSpec>, _>>()
        {
            specs.iter().try_fold(surface_to_filter, |surface, spec| {
                filters::render(
                    spec,
                    stroke_paint_source.clone(),
                    fill_paint_source.clone(),
                    surface,
                    acquired_nodes,
                    self,
                    self.get_transform(),
                    node_bbox,
                )
            })?
        } else {
            surface_to_filter
        };

        Ok(surface)
    }

    fn set_gradient(&mut self, gradient: &UserSpaceGradient) -> Result<(), cairo::Error> {
        let g = match gradient.variant {
            GradientVariant::Linear { x1, y1, x2, y2 } => {
                cairo::Gradient::clone(&cairo::LinearGradient::new(x1, y1, x2, y2))
            }

            GradientVariant::Radial {
                cx,
                cy,
                r,
                fx,
                fy,
                fr,
            } => cairo::Gradient::clone(&cairo::RadialGradient::new(fx, fy, fr, cx, cy, r)),
        };

        g.set_matrix(gradient.transform.into());
        g.set_extend(cairo::Extend::from(gradient.spread));

        for stop in &gradient.stops {
            let UnitInterval(stop_offset) = stop.offset;

            g.add_color_stop_rgba(
                stop_offset,
                f64::from(stop.rgba.red_f32()),
                f64::from(stop.rgba.green_f32()),
                f64::from(stop.rgba.blue_f32()),
                f64::from(stop.rgba.alpha_f32()),
            );
        }

        self.cr.set_source(&g)
    }

    fn set_pattern(
        &mut self,
        pattern: &UserSpacePattern,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<bool, RenderingError> {
        // Bail out early if the pattern has zero size, per the spec
        if approx_eq!(f64, pattern.width, 0.0) || approx_eq!(f64, pattern.height, 0.0) {
            return Ok(false);
        }

        // Bail out early if this pattern has a circular reference
        let pattern_node_acquired = match pattern.acquire_pattern_node(acquired_nodes) {
            Ok(n) => n,

            Err(AcquireError::CircularReference(ref node)) => {
                rsvg_log!("circular reference in element {}", node);
                return Ok(false);
            }

            _ => unreachable!(),
        };

        let pattern_node = pattern_node_acquired.get();

        let taffine = self.get_transform().pre_transform(&pattern.transform);

        let mut scwscale = (taffine.xx.powi(2) + taffine.xy.powi(2)).sqrt();
        let mut schscale = (taffine.yx.powi(2) + taffine.yy.powi(2)).sqrt();

        let pw: i32 = (pattern.width * scwscale) as i32;
        let ph: i32 = (pattern.height * schscale) as i32;

        if pw < 1 || ph < 1 {
            return Ok(false);
        }

        scwscale = f64::from(pw) / pattern.width;
        schscale = f64::from(ph) / pattern.height;

        // Apply the pattern transform
        let (affine, caffine) = if scwscale.approx_eq_cairo(1.0) && schscale.approx_eq_cairo(1.0) {
            (pattern.coord_transform, pattern.content_transform)
        } else {
            (
                pattern
                    .coord_transform
                    .pre_scale(1.0 / scwscale, 1.0 / schscale),
                pattern.content_transform.post_scale(scwscale, schscale),
            )
        };

        // Draw to another surface
        let surface = self
            .cr
            .target()
            .create_similar(cairo::Content::ColorAlpha, pw, ph)?;

        let cr_pattern = cairo::Context::new(&surface)?;

        // Set up transformations to be determined by the contents units
        cr_pattern.set_matrix(caffine.into());

        // Draw everything

        {
            let mut pattern_draw_ctx = self.nested(cr_pattern);

            pattern_draw_ctx
                .with_alpha(pattern.opacity, &mut |dc| {
                    let pattern_cascaded = CascadedValues::new_from_node(pattern_node);
                    let pattern_values = pattern_cascaded.get();

                    let elt = pattern_node.borrow_element();

                    let stacking_ctx = StackingContext::new(
                        acquired_nodes,
                        &elt,
                        Transform::identity(),
                        pattern_values,
                    );

                    dc.with_discrete_layer(
                        &stacking_ctx,
                        acquired_nodes,
                        pattern_values,
                        false,
                        None,
                        &mut |an, dc, _transform| {
                            pattern_node.draw_children(an, &pattern_cascaded, dc, false)
                        },
                    )
                })
                .map(|_| ())?;
        }

        // Set the final surface as a Cairo pattern into the Cairo context
        let pattern = cairo::SurfacePattern::create(&surface);

        if let Some(m) = affine.invert() {
            pattern.set_matrix(m.into())
        }
        pattern.set_extend(cairo::Extend::Repeat);
        pattern.set_filter(cairo::Filter::Best);
        self.cr.set_source(&pattern)?;

        Ok(true)
    }

    fn set_color(&self, rgba: cssparser::RGBA) {
        self.cr.clone().set_source_rgba(
            f64::from(rgba.red_f32()),
            f64::from(rgba.green_f32()),
            f64::from(rgba.blue_f32()),
            f64::from(rgba.alpha_f32()),
        );
    }

    fn set_paint_source(
        &mut self,
        paint_source: &UserSpacePaintSource,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<bool, RenderingError> {
        match *paint_source {
            UserSpacePaintSource::Gradient(ref gradient, _c) => {
                self.set_gradient(gradient)?;
                Ok(true)
            }
            UserSpacePaintSource::Pattern(ref pattern, c) => {
                if self.set_pattern(pattern, acquired_nodes)? {
                    Ok(true)
                } else if let Some(c) = c {
                    self.set_color(c);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            UserSpacePaintSource::SolidColor(c) => {
                self.set_color(c);
                Ok(true)
            }
            UserSpacePaintSource::None => Ok(false),
        }
    }

    /// Computes and returns a surface corresponding to the given paint server.
    pub fn get_paint_source_surface(
        &mut self,
        width: i32,
        height: i32,
        acquired_nodes: &mut AcquiredNodes<'_>,
        paint_source: &UserSpacePaintSource,
    ) -> Result<SharedImageSurface, RenderingError> {
        let mut surface = ExclusiveImageSurface::new(width, height, SurfaceType::SRgb)?;

        surface.draw::<RenderingError>(&mut |cr| {
            let mut temporary_draw_ctx = self.nested(cr);

            // FIXME: we are ignoring any error

            let had_paint_server =
                temporary_draw_ctx.set_paint_source(paint_source, acquired_nodes)?;
            if had_paint_server {
                temporary_draw_ctx.cr.paint()?;
            }

            Ok(())
        })?;

        Ok(surface.share()?)
    }

    fn stroke(
        &mut self,
        cr: &cairo::Context,
        acquired_nodes: &mut AcquiredNodes<'_>,
        paint_source: &UserSpacePaintSource,
    ) -> Result<(), RenderingError> {
        let had_paint_server = self.set_paint_source(paint_source, acquired_nodes)?;
        if had_paint_server {
            cr.stroke_preserve()?;
        }

        Ok(())
    }

    fn fill(
        &mut self,
        cr: &cairo::Context,
        acquired_nodes: &mut AcquiredNodes<'_>,
        paint_source: &UserSpacePaintSource,
    ) -> Result<(), RenderingError> {
        let had_paint_server = self.set_paint_source(paint_source, acquired_nodes)?;
        if had_paint_server {
            cr.fill_preserve()?;
        }

        Ok(())
    }

    pub fn draw_shape(
        &mut self,
        view_params: &ViewParams,
        shape: &Shape,
        stacking_ctx: &StackingContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if shape.path.is_empty() {
            return Ok(self.empty_bbox());
        }

        self.with_discrete_layer(
            stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |an, dc, transform| {
                let cr = dc.cr.clone();
                let mut path_helper =
                    PathHelper::new(&cr, *transform, &shape.path, shape.stroke.line_cap);

                if clipping {
                    if shape.is_visible {
                        cr.set_fill_rule(cairo::FillRule::from(shape.clip_rule));
                        path_helper.set()?;
                    }
                    return Ok(dc.empty_bbox());
                }

                cr.set_antialias(cairo::Antialias::from(shape.shape_rendering));

                setup_cr_for_stroke(&cr, &shape.stroke);

                cr.set_fill_rule(cairo::FillRule::from(shape.fill_rule));

                path_helper.set()?;
                let bbox = compute_stroke_and_fill_box(&cr, &shape.stroke, &shape.stroke_paint)?;

                let stroke_paint = shape.stroke_paint.to_user_space(&bbox, view_params, values);
                let fill_paint = shape.fill_paint.to_user_space(&bbox, view_params, values);

                if shape.is_visible {
                    for &target in &shape.paint_order.targets {
                        // fill and stroke operations will preserve the path.
                        // markers operation will clear the path.
                        match target {
                            PaintTarget::Fill => {
                                path_helper.set()?;
                                dc.fill(&cr, an, &fill_paint)?;
                            }

                            PaintTarget::Stroke => {
                                path_helper.set()?;
                                dc.stroke(&cr, an, &stroke_paint)?;
                            }

                            PaintTarget::Markers => {
                                path_helper.unset();
                                marker::render_markers_for_shape(shape, dc, an, clipping)?;
                            }
                        }
                    }
                }

                path_helper.unset();
                Ok(bbox)
            },
        )
    }

    fn paint_surface(
        &mut self,
        surface: &SharedImageSurface,
        width: f64,
        height: f64,
    ) -> Result<(), cairo::Error> {
        let cr = self.cr.clone();

        // We need to set extend appropriately, so can't use cr.set_source_surface().
        //
        // If extend is left at its default value (None), then bilinear scaling uses
        // transparency outside of the image producing incorrect results.
        // For example, in svg1.1/filters-blend-01-b.svgthere's a completely
        // opaque 100×1 image of a gradient scaled to 100×98 which ends up
        // transparent almost everywhere without this fix (which it shouldn't).
        let ptn = surface.to_cairo_pattern();
        ptn.set_extend(cairo::Extend::Pad);
        cr.set_source(&ptn)?;

        // Clip is needed due to extend being set to pad.
        clip_to_rectangle(&cr, &Rect::from_size(width, height));

        cr.paint()
    }

    pub fn draw_image(
        &mut self,
        image: &Image,
        stacking_ctx: &StackingContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let image_width = image.surface.width();
        let image_height = image.surface.height();
        if clipping || image.rect.is_empty() || image_width == 0 || image_height == 0 {
            return Ok(self.empty_bbox());
        }

        let image_width = f64::from(image_width);
        let image_height = f64::from(image_height);
        let vbox = ViewBox::from(Rect::from_size(image_width, image_height));

        let clip_mode = if !(image.overflow == Overflow::Auto
            || image.overflow == Overflow::Visible)
            && image.aspect.is_slice()
        {
            Some(ClipMode::ClipToViewport)
        } else {
            None
        };

        // The bounding box for <image> is decided by the values of the image's x, y, w, h
        // and not by the final computed image bounds.
        let bounds = self.empty_bbox().with_rect(image.rect);

        if image.is_visible {
            self.with_discrete_layer(
                stacking_ctx,
                acquired_nodes,
                values,
                clipping,
                None,
                &mut |_an, dc, _transform| {
                    with_saved_cr(&dc.cr.clone(), || {
                        if let Some(_params) =
                            dc.push_new_viewport(Some(vbox), image.rect, image.aspect, clip_mode)
                        {
                            dc.paint_surface(&image.surface, image_width, image_height)?;
                        }

                        Ok(bounds)
                    })
                },
            )
        } else {
            Ok(bounds)
        }
    }

    fn draw_text_span(
        &mut self,
        span: &TextSpan,
        acquired_nodes: &mut AcquiredNodes<'_>,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let path = pango_layout_to_path(span.x, span.y, &span.layout, span.gravity)?;
        if path.is_empty() {
            // Empty strings, or only-whitespace text, get turned into empty paths.
            // In that case, we really want to return "no bounds" rather than an
            // empty rectangle.
            return Ok(self.empty_bbox());
        }

        // #851 - We can't just render all text as paths for PDF; it
        // needs the actual text content so text is selectable by PDF
        // viewers.
        let can_use_text_as_path = self.cr.target().type_() != cairo::SurfaceType::Pdf;

        with_saved_cr(&self.cr.clone(), || {
            self.cr
                .set_antialias(cairo::Antialias::from(span.text_rendering));

            setup_cr_for_stroke(&self.cr, &span.stroke);

            if clipping {
                path.to_cairo(&self.cr, false)?;
                return Ok(self.empty_bbox());
            }

            path.to_cairo(&self.cr, false)?;
            let bbox =
                compute_stroke_and_fill_box(&self.cr, &span.stroke, &span.stroke_paint_source)?;
            self.cr.new_path();

            if span.is_visible {
                if let Some(ref link_target) = span.link_target {
                    self.link_tag_begin(link_target);
                }

                for &target in &span.paint_order.targets {
                    match target {
                        PaintTarget::Fill => {
                            let had_paint_server =
                                self.set_paint_source(&span.fill_paint, acquired_nodes)?;

                            if had_paint_server {
                                if can_use_text_as_path {
                                    path.to_cairo(&self.cr, false)?;
                                    self.cr.fill()?;
                                    self.cr.new_path();
                                } else {
                                    self.cr.move_to(span.x, span.y);

                                    let matrix = self.cr.matrix();

                                    let rotation_from_gravity = span.gravity.to_rotation();
                                    if !rotation_from_gravity.approx_eq_cairo(0.0) {
                                        self.cr.rotate(-rotation_from_gravity);
                                    }

                                    pangocairo::functions::update_layout(&self.cr, &span.layout);
                                    pangocairo::functions::show_layout(&self.cr, &span.layout);

                                    self.cr.set_matrix(matrix);
                                }
                            }
                        }

                        PaintTarget::Stroke => {
                            let had_paint_server =
                                self.set_paint_source(&span.stroke_paint, acquired_nodes)?;

                            if had_paint_server {
                                path.to_cairo(&self.cr, false)?;
                                self.cr.stroke()?;
                                self.cr.new_path();
                            }
                        }

                        PaintTarget::Markers => {}
                    }
                }

                if span.link_target.is_some() {
                    self.link_tag_end();
                }
            }

            Ok(bbox)
        })
    }

    pub fn draw_text(
        &mut self,
        text: &Text,
        acquired_nodes: &mut AcquiredNodes<'_>,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let mut bbox = self.empty_bbox();

        for span in &text.spans {
            let span_bbox = self.draw_text_span(span, acquired_nodes, clipping)?;
            bbox.insert(&span_bbox);
        }

        Ok(bbox)
    }

    pub fn get_snapshot(
        &self,
        width: i32,
        height: i32,
    ) -> Result<SharedImageSurface, cairo::Error> {
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

        surface.draw::<cairo::Error>(&mut |cr| {
            // TODO: apparently DrawingCtx.cr_stack is just a way to store pairs of
            // (surface, transform).  Can we turn it into a DrawingCtx.surface_stack
            // instead?  See what CSS isolation would like to call that; are the pairs just
            // stacking contexts instead, or the result of rendering stacking contexts?
            for (depth, draw) in self.cr_stack.borrow().iter().enumerate() {
                let affines = CompositingAffines::new(
                    Transform::from(draw.matrix()),
                    self.initial_viewport.transform,
                    depth,
                );

                cr.set_matrix(affines.for_snapshot.into());
                cr.set_source_surface(&draw.target(), 0.0, 0.0)?;
                cr.paint()?;
            }

            Ok(())
        })?;

        surface.share()
    }

    pub fn draw_node_to_surface(
        &mut self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        affine: Transform,
        width: i32,
        height: i32,
    ) -> Result<SharedImageSurface, RenderingError> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;

        let save_initial_viewport = self.initial_viewport;
        let save_cr = self.cr.clone();

        {
            let cr = cairo::Context::new(&surface)?;
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
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let stack_top = self.drawsub_stack.pop();

        let draw = if let Some(ref top) = stack_top {
            top == node
        } else {
            true
        };

        let res = if draw {
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
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        use_rect: Rect,
        link: &NodeId,
        clipping: bool,
        fill_paint: PaintSource,
        stroke_paint: PaintSource,
    ) -> Result<BoundingBox, RenderingError> {
        // <use> is an element that is used directly, unlike
        // <pattern>, which is used through a fill="url(#...)"
        // reference.  However, <use> will always reference another
        // element, potentially itself or an ancestor of itself (or
        // another <use> which references the first one, etc.).  So,
        // we acquire the <use> element itself so that circular
        // references can be caught.
        let _self_acquired = match acquired_nodes.acquire_ref(node) {
            Ok(n) => n,

            Err(AcquireError::CircularReference(_)) => {
                rsvg_log!("circular reference in element {}", node);
                return Ok(self.empty_bbox());
            }

            _ => unreachable!(),
        };

        let acquired = match acquired_nodes.acquire(link) {
            Ok(acquired) => acquired,

            Err(AcquireError::CircularReference(node)) => {
                rsvg_log!("circular reference in element {}", node);
                return Ok(self.empty_bbox());
            }

            Err(AcquireError::MaxReferencesExceeded) => {
                return Err(RenderingError::LimitExceeded(
                    ImplementationLimit::TooManyReferencedElements,
                ));
            }

            Err(AcquireError::InvalidLinkType(_)) => unreachable!(),

            Err(AcquireError::LinkNotFound(node_id)) => {
                rsvg_log!("element {} references nonexistent \"{}\"", node, node_id);
                return Ok(self.empty_bbox());
            }
        };

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if use_rect.is_empty() {
            return Ok(self.empty_bbox());
        }

        let child = acquired.get();

        if clipping && !element_can_be_used_inside_use_inside_clip_path(&child.borrow_element()) {
            return Ok(self.empty_bbox());
        }

        let orig_transform = self.get_transform();

        self.cr.transform(values.transform().into());

        let use_element = node.borrow_element();

        let res = if is_element_of_type!(child, Symbol) {
            // if the <use> references a <symbol>, it gets handled specially

            let elt = child.borrow_element();

            let symbol = borrow_element_as!(child, Symbol);

            let clip_mode = if !values.is_overflow()
                || (values.overflow() == Overflow::Visible
                    && elt.get_specified_values().is_overflow())
            {
                Some(ClipMode::ClipToVbox)
            } else {
                None
            };

            let stacking_ctx =
                StackingContext::new(acquired_nodes, &use_element, Transform::identity(), values);

            self.with_discrete_layer(
                &stacking_ctx,
                acquired_nodes,
                values,
                clipping,
                None,
                &mut |an, dc, _transform| {
                    let _params = dc.push_new_viewport(
                        symbol.get_viewbox(),
                        use_rect,
                        symbol.get_preserve_aspect_ratio(),
                        clip_mode,
                    );

                    child.draw_children(
                        an,
                        &CascadedValues::new_from_values(
                            child,
                            values,
                            Some(fill_paint.clone()),
                            Some(stroke_paint.clone()),
                        ),
                        dc,
                        clipping,
                    )
                },
            )
        } else {
            // otherwise the referenced node is not a <symbol>; process it generically

            let stacking_ctx = StackingContext::new(
                acquired_nodes,
                &use_element,
                Transform::new_translate(use_rect.x0, use_rect.y0),
                values,
            );

            self.with_discrete_layer(
                &stacking_ctx,
                acquired_nodes,
                values,
                clipping,
                None,
                &mut |an, dc, _transform| {
                    child.draw(
                        an,
                        &CascadedValues::new_from_values(
                            child,
                            values,
                            Some(fill_paint.clone()),
                            Some(stroke_paint.clone()),
                        ),
                        dc,
                        clipping,
                    )
                },
            )
        };

        self.cr.set_matrix(orig_transform.into());

        if let Ok(bbox) = res {
            let mut res_bbox = BoundingBox::new().with_transform(orig_transform);
            res_bbox.insert(&bbox);
            Ok(res_bbox)
        } else {
            res
        }
    }

    /// Extracts the font options for the current state of the DrawingCtx.
    ///
    /// You can use the font options later with create_pango_context().
    pub fn get_font_options(&self) -> FontOptions {
        let mut options = cairo::FontOptions::new().unwrap();
        if self.testing {
            options.set_antialias(cairo::Antialias::Gray);
        }

        options.set_hint_style(cairo::HintStyle::None);
        options.set_hint_metrics(cairo::HintMetrics::Off);

        FontOptions { options }
    }
}

/// Create a Pango context with a particular configuration.
pub fn create_pango_context(font_options: &FontOptions, transform: &Transform) -> pango::Context {
    let font_map = pangocairo::FontMap::default().unwrap();
    let context = font_map.create_context().unwrap();

    context.set_round_glyph_positions(false);

    let pango_matrix = PangoMatrix {
        xx: transform.xx,
        xy: transform.xy,
        yx: transform.yx,
        yy: transform.yy,
        x0: transform.x0,
        y0: transform.y0,
    };

    let pango_matrix_ptr: *const PangoMatrix = &pango_matrix;

    let matrix = unsafe { pango::Matrix::from_glib_none(pango_matrix_ptr) };
    context.set_matrix(Some(&matrix));

    pangocairo::functions::context_set_font_options(&context, Some(&font_options.options));

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

    context
}

/// Converts a Pango layout to a Cairo path on the specified cr starting at (x, y).
/// Does not clear the current path first.
fn pango_layout_to_cairo(
    x: f64,
    y: f64,
    layout: &pango::Layout,
    gravity: pango::Gravity,
    cr: &cairo::Context,
) {
    let rotation_from_gravity = gravity.to_rotation();
    let rotation = if !rotation_from_gravity.approx_eq_cairo(0.0) {
        Some(-rotation_from_gravity)
    } else {
        None
    };

    cr.move_to(x, y);

    let matrix = cr.matrix();
    if let Some(rot) = rotation {
        cr.rotate(rot);
    }

    pangocairo::functions::update_layout(cr, layout);
    pangocairo::functions::layout_path(cr, layout);
    cr.set_matrix(matrix);
}

/// Converts a Pango layout to a Path starting at (x, y).
pub fn pango_layout_to_path(
    x: f64,
    y: f64,
    layout: &pango::Layout,
    gravity: pango::Gravity,
) -> Result<Path, RenderingError> {
    let surface = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None)?;
    let cr = cairo::Context::new(&surface)?;

    pango_layout_to_cairo(x, y, layout, gravity, &cr);

    let cairo_path = cr.copy_path()?;
    Ok(Path::from_cairo(cairo_path))
}

// https://www.w3.org/TR/css-masking-1/#ClipPathElement
fn element_can_be_used_inside_clip_path(element: &Element) -> bool {
    matches!(
        *element,
        Element::Circle(_)
            | Element::Ellipse(_)
            | Element::Line(_)
            | Element::Path(_)
            | Element::Polygon(_)
            | Element::Polyline(_)
            | Element::Rect(_)
            | Element::Text(_)
            | Element::Use(_)
    )
}

// https://www.w3.org/TR/css-masking-1/#ClipPathElement
fn element_can_be_used_inside_use_inside_clip_path(element: &Element) -> bool {
    matches!(
        *element,
        Element::Circle(_)
            | Element::Ellipse(_)
            | Element::Line(_)
            | Element::Path(_)
            | Element::Polygon(_)
            | Element::Polyline(_)
            | Element::Rect(_)
            | Element::Text(_)
    )
}

#[derive(Debug)]
struct CompositingAffines {
    pub outside_temporary_surface: Transform,
    #[allow(unused)]
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

fn compute_stroke_and_fill_extents(
    cr: &cairo::Context,
    stroke: &Stroke,
    stroke_paint_source: &PaintSource,
) -> Result<PathExtents, RenderingError> {
    // Dropping the precision of cairo's bezier subdivision, yielding 2x
    // _rendering_ time speedups, are these rather expensive operations
    // really needed here? */
    let backup_tolerance = cr.tolerance();
    cr.set_tolerance(1.0);

    // Bounding box for fill
    //
    // Unlike the case for stroke, for fills we always compute the bounding box.
    // In GNOME we have SVGs for symbolic icons where each icon has a bounding
    // rectangle with no fill and no stroke, and inside it there are the actual
    // paths for the icon's shape.  We need to be able to compute the bounding
    // rectangle's extents, even when it has no fill nor stroke.

    let (x0, y0, x1, y1) = cr.fill_extents()?;
    let fill_extents = Some(Rect::new(x0, y0, x1, y1));

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

    let stroke_extents = if !stroke.width.approx_eq_cairo(0.0)
        && !matches!(stroke_paint_source, PaintSource::None)
    {
        let (x0, y0, x1, y1) = cr.stroke_extents()?;
        Some(Rect::new(x0, y0, x1, y1))
    } else {
        None
    };

    // objectBoundingBox

    let (x0, y0, x1, y1) = cr.path_extents()?;
    let path_extents = Some(Rect::new(x0, y0, x1, y1));

    // restore tolerance

    cr.set_tolerance(backup_tolerance);

    Ok(PathExtents {
        path_only: path_extents,
        fill: fill_extents,
        stroke: stroke_extents,
    })
}

fn compute_stroke_and_fill_box(
    cr: &cairo::Context,
    stroke: &Stroke,
    stroke_paint_source: &PaintSource,
) -> Result<BoundingBox, RenderingError> {
    let extents = compute_stroke_and_fill_extents(cr, stroke, stroke_paint_source)?;

    let ink_rect = match (extents.fill, extents.stroke) {
        (None, None) => None,
        (Some(f), None) => Some(f),
        (None, Some(s)) => Some(s),
        (Some(f), Some(s)) => Some(f.union(&s)),
    };

    let mut bbox = BoundingBox::new().with_transform(Transform::from(cr.matrix()));

    if let Some(rect) = extents.path_only {
        bbox = bbox.with_rect(rect);
    }

    if let Some(ink_rect) = ink_rect {
        bbox = bbox.with_ink_rect(ink_rect);
    }

    Ok(bbox)
}

fn setup_cr_for_stroke(cr: &cairo::Context, stroke: &Stroke) {
    cr.set_line_width(stroke.width);
    cr.set_miter_limit(stroke.miter_limit.0);
    cr.set_line_cap(cairo::LineCap::from(stroke.line_cap));
    cr.set_line_join(cairo::LineJoin::from(stroke.line_join));

    let total_length: f64 = stroke.dashes.iter().sum();

    if total_length > 0.0 {
        cr.set_dash(&stroke.dashes, stroke.dash_offset);
    } else {
        cr.set_dash(&[], 0.0);
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

impl From<cairo::Matrix> for Transform {
    #[inline]
    fn from(m: cairo::Matrix) -> Self {
        Self::new_unchecked(m.xx, m.yx, m.xy, m.yy, m.x0, m.y0)
    }
}

impl From<Transform> for cairo::Matrix {
    #[inline]
    fn from(t: Transform) -> Self {
        Self::new(t.xx, t.yx, t.xy, t.yy, t.x0, t.y0)
    }
}

/// Extents for a path in its current coordinate system.
///
/// Normally you'll want to convert this to a BoundingBox, which has knowledge about just
/// what that coordinate system is.
pub struct PathExtents {
    /// Extents of the "plain", unstroked path, or `None` if the path is empty.
    pub path_only: Option<Rect>,

    /// Extents of just the fill, or `None` if the path is empty.
    pub fill: Option<Rect>,

    /// Extents for the stroked path, or `None` if the path is empty or zero-width.
    pub stroke: Option<Rect>,
}

impl Path {
    pub fn to_cairo(
        &self,
        cr: &cairo::Context,
        is_square_linecap: bool,
    ) -> Result<(), RenderingError> {
        assert!(!self.is_empty());

        for subpath in self.iter_subpath() {
            // If a subpath is empty and the linecap is a square, then draw a square centered on
            // the origin of the subpath. See #165.
            if is_square_linecap {
                let (x, y) = subpath.origin();
                if subpath.is_zero_length() {
                    let stroke_size = 0.002;

                    cr.move_to(x - stroke_size / 2., y);
                    cr.line_to(x + stroke_size / 2., y);
                }
            }

            for cmd in subpath.iter_commands() {
                cmd.to_cairo(cr);
            }
        }

        // We check the cr's status right after feeding it a new path for a few reasons:
        //
        // * Any of the individual path commands may cause the cr to enter an error state, for
        //   example, if they come with coordinates outside of Cairo's supported range.
        //
        // * The *next* call to the cr will probably be something that actually checks the status
        //   (i.e. in cairo-rs), and we don't want to panic there.

        cr.status().map_err(|e| e.into())
    }

    /// Converts a `cairo::Path` to a librsvg `Path`.
    fn from_cairo(cairo_path: cairo::Path) -> Path {
        let mut builder = PathBuilder::default();

        // Cairo has the habit of appending a MoveTo to some paths, but we don't want a
        // path for empty text to generate that lone point.  So, strip out paths composed
        // only of MoveTo.

        if !cairo_path_is_only_move_tos(&cairo_path) {
            for segment in cairo_path.iter() {
                match segment {
                    cairo::PathSegment::MoveTo((x, y)) => builder.move_to(x, y),
                    cairo::PathSegment::LineTo((x, y)) => builder.line_to(x, y),
                    cairo::PathSegment::CurveTo((x2, y2), (x3, y3), (x4, y4)) => {
                        builder.curve_to(x2, y2, x3, y3, x4, y4)
                    }
                    cairo::PathSegment::ClosePath => builder.close_path(),
                }
            }
        }

        builder.into_path()
    }
}

fn cairo_path_is_only_move_tos(path: &cairo::Path) -> bool {
    path.iter()
        .all(|seg| matches!(seg, cairo::PathSegment::MoveTo((_, _))))
}

impl PathCommand {
    fn to_cairo(&self, cr: &cairo::Context) {
        match *self {
            PathCommand::MoveTo(x, y) => cr.move_to(x, y),
            PathCommand::LineTo(x, y) => cr.line_to(x, y),
            PathCommand::CurveTo(ref curve) => curve.to_cairo(cr),
            PathCommand::Arc(ref arc) => arc.to_cairo(cr),
            PathCommand::ClosePath => cr.close_path(),
        }
    }
}

impl EllipticalArc {
    fn to_cairo(&self, cr: &cairo::Context) {
        match self.center_parameterization() {
            ArcParameterization::CenterParameters {
                center,
                radii,
                theta1,
                delta_theta,
            } => {
                let n_segs = (delta_theta / (PI * 0.5 + 0.001)).abs().ceil() as u32;
                let d_theta = delta_theta / f64::from(n_segs);

                let mut theta = theta1;
                for _ in 0..n_segs {
                    arc_segment(center, radii, self.x_axis_rotation, theta, theta + d_theta)
                        .to_cairo(cr);
                    theta += d_theta;
                }
            }
            ArcParameterization::LineTo => {
                let (x2, y2) = self.to;
                cr.line_to(x2, y2);
            }
            ArcParameterization::Omit => {}
        }
    }
}

impl CubicBezierCurve {
    fn to_cairo(&self, cr: &cairo::Context) {
        let Self { pt1, pt2, to } = *self;
        cr.curve_to(pt1.0, pt1.1, pt2.0, pt2.1, to.0, to.1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rsvg_path_from_cairo_path() {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, 10, 10).unwrap();
        let cr = cairo::Context::new(&surface).unwrap();

        cr.move_to(1.0, 2.0);
        cr.line_to(3.0, 4.0);
        cr.curve_to(5.0, 6.0, 7.0, 8.0, 9.0, 10.0);
        cr.close_path();

        let cairo_path = cr.copy_path().unwrap();
        let path = Path::from_cairo(cairo_path);

        assert_eq!(
            path.iter().collect::<Vec<PathCommand>>(),
            vec![
                PathCommand::MoveTo(1.0, 2.0),
                PathCommand::LineTo(3.0, 4.0),
                PathCommand::CurveTo(CubicBezierCurve {
                    pt1: (5.0, 6.0),
                    pt2: (7.0, 8.0),
                    to: (9.0, 10.0),
                }),
                PathCommand::ClosePath,
                PathCommand::MoveTo(1.0, 2.0), // cairo inserts a MoveTo after ClosePath
            ],
        );
    }
}
