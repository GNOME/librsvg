//! The main context structure which drives the drawing process.

use float_cmp::approx_eq;
use locale_config::{LanguageRange, Locale};
use once_cell::sync::Lazy;
use pango::FontMapExt;
use regex::{Captures, Regex};
use std::borrow::Cow;
use std::cell::RefCell;
use std::convert::TryFrom;
use std::rc::{Rc, Weak};

use crate::accept_language::LanguageTags;
use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::dasharray::Dasharray;
use crate::document::{AcquiredNodes, NodeId};
use crate::dpi::Dpi;
use crate::error::{AcquireError, ImplementationLimit, RenderingError};
use crate::filters::{self, FilterSpec};
use crate::float_eq_cairo::ApproxEqCairo;
use crate::gradient::{GradientVariant, SpreadMethod, UserSpaceGradient};
use crate::layout::StackingContext;
use crate::length::*;
use crate::marker;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::paint_server::{PaintServer, UserSpacePaintSource};
use crate::path_builder::*;
use crate::pattern::UserSpacePattern;
use crate::properties::ComputedValues;
use crate::property_defs::{
    ClipRule, FillRule, Filter, MixBlendMode, Opacity, Overflow, PaintTarget, ShapeRendering,
    StrokeDasharray, StrokeLinecap, StrokeLinejoin, TextRendering,
};
use crate::rect::Rect;
use crate::shapes::{Markers, Shape};
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

    locale_tags: LanguageTags,

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
    acquired_nodes: &mut AcquiredNodes<'_>,
) -> Result<BoundingBox, RenderingError> {
    let (drawsub_stack, node) = match mode {
        DrawingMode::LimitToStack { node, root } => (node.ancestors().collect(), root),

        DrawingMode::OnlyNode(node) => (Vec::new(), node),
    };

    let cascaded = CascadedValues::new_from_node(&node);

    // Preserve the user's transform and use it for the outermost bounding box.  All bounds/extents
    // will be converted to this transform in the end.
    let user_transform = Transform::from(cr.get_matrix());
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
        dpi,
        measuring,
        testing,
        drawsub_stack,
    );

    let content_bbox = draw_ctx.draw_node_from_stack(&node, acquired_nodes, &cascaded, false)?;

    user_bbox.insert(&content_bbox);

    Ok(user_bbox)
}

struct SavedCr<'a> {
    draw_ctx: &'a mut DrawingCtx,
}

impl SavedCr<'_> {
    /// Saves the draw_ctx.cr, which will be restored on Drop.
    fn new(draw_ctx: &mut DrawingCtx) -> SavedCr<'_> {
        draw_ctx.cr.save();
        SavedCr { draw_ctx }
    }
}

impl Drop for SavedCr<'_> {
    fn drop(&mut self) {
        self.draw_ctx.cr.restore();
    }
}

impl Drop for DrawingCtx {
    fn drop(&mut self) {
        self.cr_stack.borrow_mut().pop();
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

impl DrawingCtx {
    fn new(
        cr: &cairo::Context,
        transform: Transform,
        viewport: Rect,
        dpi: Dpi,
        measuring: bool,
        testing: bool,
        drawsub_stack: Vec<Node>,
    ) -> DrawingCtx {
        let vbox = ViewBox::from(viewport);
        let initial_viewport = Viewport { transform, vbox };

        let viewport_stack = vec![initial_viewport];

        let locale_tags = LanguageTags::from_locale(&locale_from_environment())
            .map_err(|s| {
                rsvg_log!("could not convert locale to language tags: {}", s);
            })
            .unwrap_or_else(|_| LanguageTags::empty());

        DrawingCtx {
            initial_viewport,
            dpi,
            cr_stack: Rc::new(RefCell::new(Vec::new())),
            cr: cr.clone(),
            locale_tags,
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
            locale_tags: self.locale_tags.clone(),
            viewport_stack: self.viewport_stack.clone(),
            drawsub_stack: Vec::new(),
            measuring: self.measuring,
            testing: self.testing,
        }
    }

    pub fn locale_tags(&self) -> &LanguageTags {
        &self.locale_tags
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
    /// inside `SavedCr` scope or `draw_ctx.with_discrete_layer`.
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
            let node_transform = node
                .borrow_element()
                .get_transform()
                .post_transform(&transform);

            let cascaded = CascadedValues::new_from_node(node);

            let orig_transform = self.get_transform();
            self.cr.transform(node_transform.into());

            // here we don't push a layer because we are clipping
            let res = node.draw_children(acquired_nodes, &cascaded, self, true);

            self.cr.clip();

            self.cr.set_matrix(orig_transform.into());

            // Clipping paths do not contribute to bounding boxes, so ignore the bbox from
            // the clip path.
            res.map(|_bbox| ())
        } else {
            Ok(())
        }
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
            let view_params = self.push_coord_units(mask_units);
            let params = NormalizeParams::new(values, &view_params);
            mask.get_rect(&params)
        };

        let mask_element = mask_node.borrow_element();

        let mask_transform = mask_element.get_transform().post_transform(&transform);

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

            let mut mask_draw_ctx = self.nested(mask_cr);

            let stacking_ctx =
                StackingContext::new(acquired_nodes, &mask_element, Transform::identity(), values);

            let res = mask_draw_ctx.with_discrete_layer(
                &stacking_ctx,
                acquired_nodes,
                values,
                false,
                None,
                &mut |an, dc| mask_node.draw_children(an, &cascaded, dc, false),
            );

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
        stacking_ctx: &StackingContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        clipping: bool,
        clip_rect: Option<Rect>,
        draw_fn: &mut dyn FnMut(
            &mut AcquiredNodes<'_>,
            &mut DrawingCtx,
        ) -> Result<BoundingBox, RenderingError>,
    ) -> Result<BoundingBox, RenderingError> {
        let orig_transform = self.get_transform();

        self.cr.transform(stacking_ctx.transform.into());

        let res = if clipping {
            draw_fn(acquired_nodes, self)
        } else {
            let saved_cr = SavedCr::new(self);

            let clip_path_value = values.clip_path();

            let clip_uri = clip_path_value.0.get();

            let Opacity(UnitInterval(opacity)) = stacking_ctx.opacity;

            let affine_at_start = saved_cr.draw_ctx.get_transform();

            let (clip_in_user_space, clip_in_object_space) =
                get_clip_in_user_and_object_space(acquired_nodes, clip_uri);

            if let Some(rect) = clip_rect {
                clip_to_rectangle(&saved_cr.draw_ctx.cr, &rect);
            }

            // Here we are clipping in user space, so the bbox doesn't matter
            saved_cr.draw_ctx.clip_to_node(
                &clip_in_user_space,
                acquired_nodes,
                &saved_cr.draw_ctx.empty_bbox(),
            )?;

            let is_opaque = approx_eq!(f64, opacity, 1.0);
            let needs_temporary_surface = !(is_opaque
                && stacking_ctx.filter == Filter::None
                && stacking_ctx.mask.is_none()
                && values.mix_blend_mode() == MixBlendMode::Normal
                && clip_in_object_space.is_none());

            if needs_temporary_surface {
                // Compute our assortment of affines

                let affines = CompositingAffines::new(
                    affine_at_start,
                    saved_cr.draw_ctx.initial_transform_with_offset(),
                    saved_cr.draw_ctx.cr_stack.borrow().len(),
                );

                // Create temporary surface and its cr

                let cr = match stacking_ctx.filter {
                    Filter::None => cairo::Context::new(
                        &saved_cr
                            .draw_ctx
                            .create_similar_surface_for_toplevel_viewport(
                                &saved_cr.draw_ctx.cr.get_target(),
                            )?,
                    ),
                    Filter::List(_) => cairo::Context::new(
                        &*saved_cr.draw_ctx.create_surface_for_toplevel_viewport()?,
                    ),
                };

                cr.set_matrix(affines.for_temporary_surface.into());

                let (source_surface, mut res, bbox) = {
                    let mut temporary_draw_ctx = saved_cr.draw_ctx.nested(cr);

                    // Draw!

                    let res = draw_fn(acquired_nodes, &mut temporary_draw_ctx);

                    let bbox = if let Ok(ref bbox) = res {
                        *bbox
                    } else {
                        BoundingBox::new().with_transform(affines.for_temporary_surface)
                    };

                    // Filter

                    let surface_to_filter = SharedImageSurface::copy_from_surface(
                        &cairo::ImageSurface::try_from(temporary_draw_ctx.cr.get_target()).unwrap(),
                    )?;

                    (
                        temporary_draw_ctx.run_filters(
                            surface_to_filter,
                            &stacking_ctx.filter,
                            acquired_nodes,
                            &stacking_ctx.element_name,
                            values,
                            bbox,
                        )?,
                        res,
                        bbox,
                    )
                };

                // Set temporary surface as source

                saved_cr.draw_ctx.cr.set_matrix(affines.compositing.into());
                source_surface.set_as_source_surface(&saved_cr.draw_ctx.cr, 0.0, 0.0);

                // Clip

                saved_cr
                    .draw_ctx
                    .cr
                    .set_matrix(affines.outside_temporary_surface.into());
                saved_cr
                    .draw_ctx
                    .clip_to_node(&clip_in_object_space, acquired_nodes, &bbox)?;

                // Mask

                if let Some(ref mask_node) = stacking_ctx.mask {
                    res = res.and_then(|bbox| {
                        saved_cr
                            .draw_ctx
                            .generate_cairo_mask(
                                mask_node,
                                affines.for_temporary_surface,
                                &bbox,
                                acquired_nodes,
                            )
                            .map(|mask_surf| {
                                if let Some(surf) = mask_surf {
                                    saved_cr.draw_ctx.cr.set_matrix(affines.compositing.into());
                                    saved_cr.draw_ctx.cr.mask_surface(&surf, 0.0, 0.0);
                                }
                            })
                            .map(|_: ()| bbox)
                    });
                } else {
                    // No mask, so composite the temporary surface

                    saved_cr.draw_ctx.cr.set_matrix(affines.compositing.into());
                    saved_cr
                        .draw_ctx
                        .cr
                        .set_operator(values.mix_blend_mode().into());

                    if opacity < 1.0 {
                        saved_cr.draw_ctx.cr.paint_with_alpha(opacity);
                    } else {
                        saved_cr.draw_ctx.cr.paint();
                    }
                }

                saved_cr.draw_ctx.cr.set_matrix(affine_at_start.into());

                res
            } else {
                draw_fn(acquired_nodes, saved_cr.draw_ctx)
            }
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

    fn initial_transform_with_offset(&self) -> Transform {
        let rect = self.toplevel_viewport();

        self.initial_viewport
            .transform
            .pre_translate(rect.x0, rect.y0)
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
            self.cr.pop_group_to_source();
            self.cr.paint_with_alpha(o);
        } else {
            res = draw_fn(self);
        }

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
        surface_to_filter: SharedImageSurface,
        filters: &Filter,
        acquired_nodes: &mut AcquiredNodes<'_>,
        node_name: &str,
        values: &ComputedValues,
        node_bbox: BoundingBox,
    ) -> Result<SharedImageSurface, RenderingError> {
        let stroke_paint_source = Rc::new(
            values
                .stroke()
                .0
                .resolve(acquired_nodes, values.stroke_opacity().0, values.color().0)?
                .to_user_space(&node_bbox, self, values),
        );

        let fill_paint_source = Rc::new(
            values
                .fill()
                .0
                .resolve(acquired_nodes, values.fill_opacity().0, values.color().0)?
                .to_user_space(&node_bbox, self, values),
        );

        let surface = match filters {
            Filter::None => surface_to_filter,
            Filter::List(filter_list) => {
                if let Ok(specs) = filter_list
                    .iter()
                    .map(|filter_value| {
                        filter_value.to_filter_spec(acquired_nodes, values, self, node_name)
                    })
                    .collect::<Result<Vec<FilterSpec>, _>>()
                {
                    specs.iter().try_fold(surface_to_filter, |surface, spec| {
                        filters::render(
                            &spec,
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
                }
            }
        };

        Ok(surface)
    }

    fn set_gradient(self: &mut DrawingCtx, gradient: &UserSpaceGradient) {
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

        let cr = self.cr.clone();
        cr.set_source(&g);
    }

    fn set_pattern(
        &mut self,
        pattern: &UserSpacePattern,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<bool, RenderingError> {
        if approx_eq!(f64, pattern.width, 0.0) || approx_eq!(f64, pattern.height, 0.0) {
            return Ok(false);
        }

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
            .get_target()
            .create_similar(cairo::Content::ColorAlpha, pw, ph)?;

        let cr_pattern = cairo::Context::new(&surface);

        // Set up transformations to be determined by the contents units
        cr_pattern.set_matrix(caffine.into());

        // Draw everything

        {
            let mut pattern_draw_ctx = self.nested(cr_pattern);

            pattern_draw_ctx
                .with_alpha(pattern.opacity, &mut |dc| {
                    let pattern_cascaded =
                        CascadedValues::new_from_node(&pattern.node_with_children);
                    let pattern_values = pattern_cascaded.get();

                    let elt = pattern.node_with_children.borrow_element();

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
                        &mut |an, dc| {
                            pattern.node_with_children.draw_children(
                                an,
                                &pattern_cascaded,
                                dc,
                                false,
                            )
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
        self.cr.set_source(&pattern);

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
                self.set_gradient(gradient);
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
    ) -> Result<SharedImageSurface, cairo::Error> {
        let mut surface = ExclusiveImageSurface::new(width, height, SurfaceType::SRgb)?;

        surface.draw(&mut |cr| {
            let mut temporary_draw_ctx = self.nested(cr);

            // FIXME: we are ignoring any error

            let _ = temporary_draw_ctx
                .set_paint_source(paint_source, acquired_nodes)
                .map(|had_paint_server| {
                    if had_paint_server {
                        temporary_draw_ctx.cr.paint();
                    }
                });

            Ok(())
        })?;

        surface.share()
    }

    fn setup_cr_for_stroke(&self, cr: &cairo::Context, values: &ComputedValues) {
        let view_params = self.get_view_params();
        let params = NormalizeParams::new(values, &view_params);

        cr.set_line_width(values.stroke_width().0.to_user(&params));
        cr.set_miter_limit(values.stroke_miterlimit().0);
        cr.set_line_cap(cairo::LineCap::from(values.stroke_line_cap()));
        cr.set_line_join(cairo::LineJoin::from(values.stroke_line_join()));

        if let StrokeDasharray(Dasharray::Array(ref dashes)) = values.stroke_dasharray() {
            let normalized_dashes: Vec<f64> = dashes.iter().map(|l| l.to_user(&params)).collect();

            let total_length = normalized_dashes.iter().fold(0.0, |acc, &len| acc + len);

            if total_length > 0.0 {
                let offset = values.stroke_dashoffset().0.to_user(&params);
                cr.set_dash(&normalized_dashes, offset);
            } else {
                cr.set_dash(&[], 0.0);
            }
        }
    }

    fn stroke(
        &mut self,
        cr: &cairo::Context,
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        let paint_source = values
            .stroke()
            .0
            .resolve(acquired_nodes, values.stroke_opacity().0, values.color().0)?
            .to_user_space(bbox, self, values);

        self.set_paint_source(&paint_source, acquired_nodes)
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
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        bbox: &BoundingBox,
    ) -> Result<(), RenderingError> {
        let paint_source = values
            .fill()
            .0
            .resolve(acquired_nodes, values.fill_opacity().0, values.color().0)?
            .to_user_space(bbox, self, values);

        self.set_paint_source(&paint_source, acquired_nodes)
            .map(|had_paint_server| {
                if had_paint_server {
                    cr.fill_preserve();
                }
            })?;

        Ok(())
    }

    pub fn draw_shape(
        &mut self,
        shape: &Shape,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        values: &ComputedValues,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if shape.path.is_empty() {
            return Ok(self.empty_bbox());
        }

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(acquired_nodes, &elt, elt.get_transform(), values);

        self.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |an, dc| {
                let cr = dc.cr.clone();
                let transform = dc.get_transform();
                let mut path_helper =
                    PathHelper::new(&cr, transform, &shape.path, values.stroke_line_cap());

                if clipping {
                    if values.is_visible() {
                        cr.set_fill_rule(cairo::FillRule::from(values.clip_rule()));
                        path_helper.set()?;
                    }
                    return Ok(dc.empty_bbox());
                }

                cr.set_antialias(cairo::Antialias::from(values.shape_rendering()));
                dc.setup_cr_for_stroke(&cr, values);

                cr.set_fill_rule(cairo::FillRule::from(values.fill_rule()));

                let mut bounding_box: Option<BoundingBox> = None;
                path_helper.unset();

                let view_params = dc.get_view_params();

                for &target in &values.paint_order().targets {
                    // fill and stroke operations will preserve the path.
                    // markers operation will clear the path.
                    match target {
                        PaintTarget::Fill | PaintTarget::Stroke => {
                            path_helper.set()?;
                            let bbox = bounding_box.get_or_insert_with(|| {
                                compute_stroke_and_fill_box(&cr, &values, &view_params)
                            });

                            if values.is_visible() {
                                if target == PaintTarget::Stroke {
                                    dc.stroke(&cr, an, values, &bbox)?;
                                } else {
                                    dc.fill(&cr, an, values, &bbox)?;
                                }
                            }
                        }
                        PaintTarget::Markers if shape.markers == Markers::Yes => {
                            path_helper.unset();
                            marker::render_markers_for_path(&shape.path, dc, an, values, clipping)?;
                        }
                        _ => {}
                    }
                }

                path_helper.unset();
                Ok(bounding_box.unwrap())
            },
        )
    }

    fn paint_surface(&mut self, surface: &SharedImageSurface, width: f64, height: f64) {
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
        cr.set_source(&ptn);

        // Clip is needed due to extend being set to pad.
        clip_to_rectangle(&cr, &Rect::from_size(width, height));

        cr.paint();
    }

    pub fn draw_image(
        &mut self,
        surface: &SharedImageSurface,
        rect: Rect,
        aspect: AspectRatio,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
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

        let elt = node.borrow_element();

        let stacking_ctx = StackingContext::new(acquired_nodes, &elt, elt.get_transform(), values);

        self.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |_an, dc| {
                let saved_cr = SavedCr::new(dc);

                if let Some(_params) =
                    saved_cr
                        .draw_ctx
                        .push_new_viewport(Some(vbox), rect, aspect, clip_mode)
                {
                    if values.is_visible() {
                        saved_cr
                            .draw_ctx
                            .paint_surface(surface, image_width, image_height);
                    }
                }

                // The bounding box for <image> is decided by the values of x, y, w, h
                // and not by the final computed image bounds.
                Ok(saved_cr.draw_ctx.empty_bbox().with_rect(rect))
            },
        )
    }

    pub fn draw_text(
        &mut self,
        layout: &pango::Layout,
        x: f64,
        y: f64,
        acquired_nodes: &mut AcquiredNodes<'_>,
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

        let saved_cr = SavedCr::new(self);

        let cr = saved_cr.draw_ctx.cr.clone();

        cr.set_antialias(cairo::Antialias::from(values.text_rendering()));
        saved_cr.draw_ctx.setup_cr_for_stroke(&cr, &values);
        cr.move_to(x, y);

        let rotation = gravity.to_rotation();
        if !rotation.approx_eq_cairo(0.0) {
            cr.rotate(-rotation);
        }

        let res = if !clipping {
            let paint_source = values
                .fill()
                .0
                .resolve(acquired_nodes, values.fill_opacity().0, values.color().0)?
                .to_user_space(&bbox, saved_cr.draw_ctx, values);

            saved_cr
                .draw_ctx
                .set_paint_source(&paint_source, acquired_nodes)
                .map(|had_paint_server| {
                    if had_paint_server {
                        pangocairo::functions::update_layout(&cr, &layout);
                        if values.is_visible() {
                            pangocairo::functions::show_layout(&cr, &layout);
                        }
                    };
                })
        } else {
            Ok(())
        };

        if res.is_ok() {
            let mut need_layout_path = clipping;

            let res = if !clipping {
                let paint_source = values
                    .stroke()
                    .0
                    .resolve(acquired_nodes, values.stroke_opacity().0, values.color().0)?
                    .to_user_space(&bbox, saved_cr.draw_ctx, values);

                saved_cr
                    .draw_ctx
                    .set_paint_source(&paint_source, acquired_nodes)
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
                    bbox.insert(&ib);
                    if values.is_visible() {
                        cr.stroke();
                    }
                }
            }
        }

        res.map(|_: ()| bbox)
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

        surface.draw(&mut |cr| {
            for (depth, draw) in self.cr_stack.borrow().iter().enumerate() {
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

        let orig_transform = self.get_transform();

        self.cr
            .transform(node.borrow_element().get_transform().into());

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
                &mut |an, dc| {
                    child.draw(
                        an,
                        &CascadedValues::new_from_values(&child, values),
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
    acquired_nodes: &mut AcquiredNodes<'_>,
    clip_uri: Option<&NodeId>,
) -> (Option<Node>, Option<Node>) {
    clip_uri
        .and_then(|node_id| {
            acquired_nodes
                .acquire(node_id)
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
    view_params: &ViewParams,
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

    let stroke_width = values
        .stroke_width()
        .0
        .to_user(&NormalizeParams::new(values, view_params));

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
    matches!(gravity, pango::Gravity::East | pango::Gravity::West)
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
