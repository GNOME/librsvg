//! The main context structure which drives the drawing process.

use float_cmp::approx_eq;
use glib::translate::*;
use pango::ffi::PangoMatrix;
use pango::prelude::FontMapExt;
use regex::{Captures, Regex};
use std::cell::RefCell;
use std::convert::TryFrom;
use std::f64::consts::*;
use std::rc::Rc;
use std::{borrow::Cow, sync::OnceLock};

use crate::accept_language::UserLanguage;
use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::color::color_to_rgba;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNodes, NodeId};
use crate::dpi::Dpi;
use crate::element::{Element, ElementData};
use crate::error::{AcquireError, ImplementationLimit, InternalRenderingError};
use crate::filters::{self, FilterSpec};
use crate::float_eq_cairo::ApproxEqCairo;
use crate::gradient::{GradientVariant, SpreadMethod, UserSpaceGradient};
use crate::layout::{
    Filter, Image, Layer, LayerKind, Shape, StackingContext, Stroke, Text, TextSpan,
};
use crate::length::*;
use crate::marker;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::paint_server::{PaintSource, UserSpacePaintSource};
use crate::path_builder::*;
use crate::pattern::UserSpacePattern;
use crate::properties::{
    ClipRule, ComputedValues, FillRule, ImageRendering, MaskType, MixBlendMode, Opacity, Overflow,
    PaintTarget, ShapeRendering, StrokeLinecap, StrokeLinejoin, TextRendering,
};
use crate::rect::{rect_to_transform, IRect, Rect};
use crate::rsvg_log;
use crate::session::Session;
use crate::surface_utils::shared_surface::{
    ExclusiveImageSurface, Interpolation, SharedImageSurface, SurfaceType,
};
use crate::transform::{Transform, ValidTransform};
use crate::unit_interval::UnitInterval;
use crate::viewbox::ViewBox;
use crate::{borrow_element_as, is_element_of_type};

/// Opaque font options for a DrawingCtx.
///
/// This is used for DrawingCtx::create_pango_context.
pub struct FontOptions {
    options: cairo::FontOptions,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ClipMode {
    ClipToViewport,
    NoClip,
}

/// Set path on the cairo context, or clear it.
/// This helper object keeps track whether the path has been set already,
/// so that it isn't recalculated every so often.
struct PathHelper<'a> {
    cr: &'a cairo::Context,
    transform: ValidTransform,
    path: &'a Path,
    is_square_linecap: bool,
    has_path: Option<bool>,
}

impl<'a> PathHelper<'a> {
    pub fn new(
        cr: &'a cairo::Context,
        transform: ValidTransform,
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

    pub fn set(&mut self) -> Result<(), InternalRenderingError> {
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

/// Holds the size of the current viewport in the user's coordinate system.
#[derive(Clone)]
pub struct Viewport {
    pub dpi: Dpi,

    /// Corners of the current coordinate space.
    pub vbox: ViewBox,

    /// The viewport's coordinate system, or "user coordinate system" in SVG terms.
    transform: Transform,
}

impl Viewport {
    /// FIXME: this is just used in Handle::with_height_to_user(), and in length.rs's test suite.
    /// Find a way to do this without involving a default identity transform.
    pub fn new(dpi: Dpi, view_box_width: f64, view_box_height: f64) -> Viewport {
        Viewport {
            dpi,
            vbox: ViewBox::from(Rect::from_size(view_box_width, view_box_height)),
            transform: Default::default(),
        }
    }

    /// Creates a new viewport suitable for a certain kind of units.
    ///
    /// For `objectBoundingBox`, CSS lengths which are in percentages
    /// refer to the size of the current viewport.  Librsvg implements
    /// that by keeping the same current transformation matrix, and
    /// setting a viewport size of (1.0, 1.0).
    ///
    /// For `userSpaceOnUse`, we just duplicate the current viewport,
    /// since that kind of units means to use the current coordinate
    /// system unchanged.
    pub fn with_units(&self, units: CoordUnits) -> Viewport {
        match units {
            CoordUnits::ObjectBoundingBox => Viewport {
                dpi: self.dpi,
                vbox: ViewBox::from(Rect::from_size(1.0, 1.0)),
                transform: self.transform,
            },

            CoordUnits::UserSpaceOnUse => Viewport {
                dpi: self.dpi,
                vbox: self.vbox,
                transform: self.transform,
            },
        }
    }

    /// Returns a viewport with a new size for normalizing `Length` values.
    pub fn with_view_box(&self, width: f64, height: f64) -> Viewport {
        Viewport {
            dpi: self.dpi,
            vbox: ViewBox::from(Rect::from_size(width, height)),
            transform: self.transform,
        }
    }
}

pub struct DrawingCtx {
    session: Session,

    initial_viewport: Viewport,

    dpi: Dpi,

    cr_stack: Rc<RefCell<Vec<cairo::Context>>>,
    cr: cairo::Context,

    user_language: UserLanguage,

    drawsub_stack: Vec<Node>,

    svg_nesting: SvgNesting,

    measuring: bool,
    testing: bool,
}

pub enum DrawingMode {
    LimitToStack { node: Node, root: Node },

    OnlyNode(Node),
}

/// Whether an SVG document is being rendered standalone or referenced from an `<image>` element.
///
/// Normally, the coordinate system used when rendering a toplevel SVG is determined from the
/// initial viewport and the `<svg>` element's `viewBox` and `preserveAspectRatio` attributes.
/// However, when an SVG document is referenced from an `<image>` element, as in `<image href="foo.svg"/>`,
/// its `preserveAspectRatio` needs to be ignored so that the one from the `<image>` element can
/// be used instead.  This lets the parent document (the one with the `<image>` element) specify
/// how it wants the child SVG to be scaled into the viewport.
#[derive(Copy, Clone)]
pub enum SvgNesting {
    Standalone,
    ReferencedFromImageElement,
}

/// The toplevel drawing routine.
///
/// This creates a DrawingCtx internally and starts drawing at the specified `node`.
pub fn draw_tree(
    session: Session,
    mode: DrawingMode,
    cr: &cairo::Context,
    viewport_rect: Rect,
    user_language: &UserLanguage,
    dpi: Dpi,
    svg_nesting: SvgNesting,
    measuring: bool,
    testing: bool,
    acquired_nodes: &mut AcquiredNodes<'_>,
) -> Result<BoundingBox, InternalRenderingError> {
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
    let transform = user_transform.pre_translate(viewport_rect.x0, viewport_rect.y0);

    // Here we exit immediately if the transform is not valid, since we are in the
    // toplevel drawing function.  Downstream cases would simply not render the current
    // element and ignore the error.
    let valid_transform = ValidTransform::try_from(transform)?;
    cr.set_matrix(valid_transform.into());

    // Per the spec, so the viewport has (0, 0) as upper-left.
    let viewport_rect = viewport_rect.translate((-viewport_rect.x0, -viewport_rect.y0));
    let initial_viewport = Viewport {
        dpi,
        vbox: ViewBox::from(viewport_rect),
        transform,
    };

    let mut draw_ctx = DrawingCtx::new(
        session,
        cr,
        &initial_viewport,
        user_language.clone(),
        dpi,
        svg_nesting,
        measuring,
        testing,
        drawsub_stack,
    );

    let content_bbox = draw_ctx.draw_node_from_stack(
        &node,
        acquired_nodes,
        &cascaded,
        &initial_viewport,
        false,
    )?;

    user_bbox.insert(&content_bbox);

    Ok(user_bbox)
}

pub fn with_saved_cr<O, F>(cr: &cairo::Context, f: F) -> Result<O, InternalRenderingError>
where
    F: FnOnce() -> Result<O, InternalRenderingError>,
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
        session: Session,
        cr: &cairo::Context,
        initial_viewport: &Viewport,
        user_language: UserLanguage,
        dpi: Dpi,
        svg_nesting: SvgNesting,
        measuring: bool,
        testing: bool,
        drawsub_stack: Vec<Node>,
    ) -> DrawingCtx {
        DrawingCtx {
            session,
            initial_viewport: initial_viewport.clone(),
            dpi,
            cr_stack: Rc::new(RefCell::new(Vec::new())),
            cr: cr.clone(),
            user_language,
            drawsub_stack,
            svg_nesting,
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
            session: self.session.clone(),
            initial_viewport: self.initial_viewport.clone(),
            dpi: self.dpi,
            cr_stack,
            cr,
            user_language: self.user_language.clone(),
            drawsub_stack: self.drawsub_stack.clone(),
            svg_nesting: self.svg_nesting,
            measuring: self.measuring,
            testing: self.testing,
        }
    }

    pub fn session(&self) -> &Session {
        &self.session
    }

    pub fn user_language(&self) -> &UserLanguage {
        &self.user_language
    }

    pub fn toplevel_viewport(&self) -> Rect {
        *self.initial_viewport.vbox
    }

    /// Gets the transform that will be used on the target surface,
    /// whether using an isolated stacking context or not.
    ///
    /// This is only used in the text code, and we should probably try
    /// to remove it.
    pub fn get_transform_for_stacking_ctx(
        &self,
        stacking_ctx: &StackingContext,
        clipping: bool,
    ) -> Result<ValidTransform, InternalRenderingError> {
        if stacking_ctx.should_isolate() && !clipping {
            let affines = CompositingAffines::new(
                *self.get_transform(),
                self.initial_viewport.transform,
                self.cr_stack.borrow().len(),
            );

            Ok(ValidTransform::try_from(affines.for_temporary_surface)?)
        } else {
            Ok(self.get_transform())
        }
    }

    pub fn svg_nesting(&self) -> SvgNesting {
        self.svg_nesting
    }

    pub fn is_measuring(&self) -> bool {
        self.measuring
    }

    pub fn is_testing(&self) -> bool {
        self.testing
    }

    pub fn get_transform(&self) -> ValidTransform {
        let t = Transform::from(self.cr.matrix());
        ValidTransform::try_from(t)
            .expect("Cairo should already have checked that its current transform is valid")
    }

    pub fn empty_bbox(&self) -> BoundingBox {
        BoundingBox::new().with_transform(*self.get_transform())
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
        (width.ceil().abs() as i32, height.ceil().abs() as i32)
    }

    pub fn create_surface_for_toplevel_viewport(
        &self,
    ) -> Result<cairo::ImageSurface, InternalRenderingError> {
        let (w, h) = self.size_for_temporary_surface();

        Ok(cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?)
    }

    fn create_similar_surface_for_toplevel_viewport(
        &self,
        surface: &cairo::Surface,
    ) -> Result<cairo::Surface, InternalRenderingError> {
        let (w, h) = self.size_for_temporary_surface();

        Ok(cairo::Surface::create_similar(
            surface,
            cairo::Content::ColorAlpha,
            w,
            h,
        )?)
    }

    /// Creates a new coordinate space inside a viewport and sets a clipping rectangle.
    ///
    /// Note that this actually changes the `draw_ctx.cr`'s transformation to match
    /// the new coordinate space, but the old one is not restored after the
    /// result's `Viewport` is dropped.  Thus, this function must be called
    /// inside `with_saved_cr` or `draw_ctx.with_discrete_layer`.
    pub fn push_new_viewport(
        &self,
        current_viewport: &Viewport,
        vbox: Option<ViewBox>,
        viewport_rect: Rect,
        preserve_aspect_ratio: AspectRatio,
        clip_mode: ClipMode,
    ) -> Option<Viewport> {
        if let ClipMode::ClipToViewport = clip_mode {
            clip_to_rectangle(&self.cr, &viewport_rect);
        }

        preserve_aspect_ratio
            .viewport_to_viewbox_transform(vbox, &viewport_rect)
            .unwrap_or_else(|_e| {
                match vbox {
                    None => unreachable!(
                        "viewport_to_viewbox_transform only returns errors when vbox != None"
                    ),
                    Some(v) => {
                        rsvg_log!(
                            self.session,
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

                Viewport {
                    dpi: self.dpi,
                    vbox: vbox.unwrap_or(current_viewport.vbox),
                    transform: current_viewport.transform.post_transform(&t),
                }
            })
    }

    fn clip_to_node(
        &mut self,
        clip_node: &Option<Node>,
        acquired_nodes: &mut AcquiredNodes<'_>,
        viewport: &Viewport,
        bbox: &BoundingBox,
    ) -> Result<(), InternalRenderingError> {
        if clip_node.is_none() {
            return Ok(());
        }

        let node = clip_node.as_ref().unwrap();
        let units = borrow_element_as!(node, ClipPath).get_units();

        if let Ok(transform) = rect_to_transform(&bbox.rect, units) {
            let cascaded = CascadedValues::new_from_node(node);
            let values = cascaded.get();

            let node_transform = values.transform().post_transform(&transform);
            let transform_for_clip = ValidTransform::try_from(node_transform)?;

            let orig_transform = self.get_transform();
            self.cr.transform(transform_for_clip.into());

            for child in node.children().filter(|c| {
                c.is_element() && element_can_be_used_inside_clip_path(&c.borrow_element())
            }) {
                child.draw(
                    acquired_nodes,
                    &CascadedValues::clone_with_node(&cascaded, &child),
                    viewport,
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
        viewport: &Viewport,
        transform: Transform,
        bbox: &BoundingBox,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<Option<cairo::ImageSurface>, InternalRenderingError> {
        if bbox.rect.is_none() {
            // The node being masked is empty / doesn't have a
            // bounding box, so there's nothing to mask!
            return Ok(None);
        }

        let _mask_acquired = match acquired_nodes.acquire_ref(mask_node) {
            Ok(n) => n,

            Err(AcquireError::CircularReference(_)) => {
                rsvg_log!(self.session, "circular reference in element {}", mask_node);
                return Ok(None);
            }

            _ => unreachable!(),
        };

        let mask_element = mask_node.borrow_element();
        let mask = borrow_element_as!(mask_node, Mask);

        let bbox_rect = bbox.rect.as_ref().unwrap();

        let cascaded = CascadedValues::new_from_node(mask_node);
        let values = cascaded.get();

        let mask_units = mask.get_units();

        let mask_rect = {
            let params = NormalizeParams::new(values, &viewport.with_units(mask_units));
            mask.get_rect(&params)
        };

        let mask_transform = values.transform().post_transform(&transform);
        let transform_for_mask = ValidTransform::try_from(mask_transform)?;

        let mask_content_surface = self.create_surface_for_toplevel_viewport()?;

        // Use a scope because mask_cr needs to release the
        // reference to the surface before we access the pixels
        {
            let mask_cr = cairo::Context::new(&mask_content_surface)?;
            mask_cr.set_matrix(transform_for_mask.into());

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
                mask_cr.transform(ValidTransform::try_from(bbtransform)?.into());
            }

            let mask_viewport = viewport.with_units(mask.get_content_units());

            let mut mask_draw_ctx = self.nested(mask_cr);

            let stacking_ctx = StackingContext::new(
                self.session(),
                acquired_nodes,
                &mask_element,
                Transform::identity(),
                None,
                values,
            );

            rsvg_log!(self.session, "(mask {}", mask_element);

            let res = mask_draw_ctx.with_discrete_layer(
                &stacking_ctx,
                acquired_nodes,
                &mask_viewport,
                false,
                &mut |an, dc| mask_node.draw_children(an, &cascaded, &mask_viewport, dc, false),
            );

            rsvg_log!(self.session, ")");

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
        viewport: &Viewport,
        clipping: bool,
        draw_fn: &mut dyn FnMut(
            &mut AcquiredNodes<'_>,
            &mut DrawingCtx,
        ) -> Result<BoundingBox, InternalRenderingError>,
    ) -> Result<BoundingBox, InternalRenderingError> {
        let stacking_ctx_transform = ValidTransform::try_from(stacking_ctx.transform)?;

        let orig_transform = self.get_transform();
        self.cr.transform(stacking_ctx_transform.into());

        let res = if clipping {
            draw_fn(acquired_nodes, self)
        } else {
            with_saved_cr(&self.cr.clone(), || {
                if let Some(ref link_target) = stacking_ctx.link_target {
                    self.link_tag_begin(link_target);
                }

                let Opacity(UnitInterval(opacity)) = stacking_ctx.opacity;

                let affine_at_start = self.get_transform();

                if let Some(rect) = stacking_ctx.clip_rect.as_ref() {
                    clip_to_rectangle(&self.cr, rect);
                }

                // Here we are clipping in user space, so the bbox doesn't matter
                self.clip_to_node(
                    &stacking_ctx.clip_in_user_space,
                    acquired_nodes,
                    viewport,
                    &self.empty_bbox(),
                )?;

                let should_isolate = stacking_ctx.should_isolate();

                let res = if should_isolate {
                    // Compute our assortment of affines

                    let affines = CompositingAffines::new(
                        *affine_at_start,
                        self.initial_viewport.transform,
                        self.cr_stack.borrow().len(),
                    );

                    // Create temporary surface and its cr

                    let cr = match stacking_ctx.filter {
                        None => cairo::Context::new(
                            &self
                                .create_similar_surface_for_toplevel_viewport(&self.cr.target())?,
                        )?,
                        Some(_) => {
                            cairo::Context::new(self.create_surface_for_toplevel_viewport()?)?
                        }
                    };

                    cr.set_matrix(ValidTransform::try_from(affines.for_temporary_surface)?.into());

                    let (source_surface, mut res, bbox) = {
                        let mut temporary_draw_ctx = self.nested(cr);

                        // Draw!

                        let res = draw_fn(acquired_nodes, &mut temporary_draw_ctx);

                        let bbox = if let Ok(ref bbox) = res {
                            *bbox
                        } else {
                            BoundingBox::new().with_transform(affines.for_temporary_surface)
                        };

                        if let Some(ref filter) = stacking_ctx.filter {
                            let surface_to_filter = SharedImageSurface::copy_from_surface(
                                &cairo::ImageSurface::try_from(temporary_draw_ctx.cr.target())
                                    .unwrap(),
                            )?;

                            let stroke_paint_source =
                                Rc::new(filter.stroke_paint_source.to_user_space(
                                    &bbox.rect,
                                    viewport,
                                    &filter.normalize_values,
                                ));
                            let fill_paint_source =
                                Rc::new(filter.fill_paint_source.to_user_space(
                                    &bbox.rect,
                                    viewport,
                                    &filter.normalize_values,
                                ));

                            // Filter functions (like "blend()", not the <filter> element) require
                            // being resolved in userSpaceonUse units, since that is the default
                            // for primitive_units.  So, get the corresponding NormalizeParams
                            // here and pass them down.
                            let user_space_params = NormalizeParams::from_values(
                                &filter.normalize_values,
                                &viewport.with_units(CoordUnits::UserSpaceOnUse),
                            );

                            let filtered_surface = temporary_draw_ctx
                                .run_filters(
                                    viewport,
                                    surface_to_filter,
                                    filter,
                                    acquired_nodes,
                                    &stacking_ctx.element_name,
                                    &user_space_params,
                                    stroke_paint_source,
                                    fill_paint_source,
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

                    self.cr
                        .set_matrix(ValidTransform::try_from(affines.compositing)?.into());
                    self.cr.set_source_surface(&source_surface, 0.0, 0.0)?;

                    // Clip

                    self.cr.set_matrix(
                        ValidTransform::try_from(affines.outside_temporary_surface)?.into(),
                    );
                    self.clip_to_node(
                        &stacking_ctx.clip_in_object_space,
                        acquired_nodes,
                        viewport,
                        &bbox,
                    )?;

                    // Mask

                    if let Some(ref mask_node) = stacking_ctx.mask {
                        res = res.and_then(|bbox| {
                            self.generate_cairo_mask(
                                mask_node,
                                viewport,
                                affines.for_temporary_surface,
                                &bbox,
                                acquired_nodes,
                            )
                            .and_then(|mask_surf| {
                                if let Some(surf) = mask_surf {
                                    self.cr.push_group();

                                    self.cr.set_matrix(
                                        ValidTransform::try_from(affines.compositing)?.into(),
                                    );
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

                        self.cr
                            .set_matrix(ValidTransform::try_from(affines.compositing)?.into());
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
                    draw_fn(acquired_nodes, self)
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
        draw_fn: &mut dyn FnMut(&mut DrawingCtx) -> Result<BoundingBox, InternalRenderingError>,
    ) -> Result<BoundingBox, InternalRenderingError> {
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
        viewport: &Viewport,
        surface_to_filter: SharedImageSurface,
        filter: &Filter,
        acquired_nodes: &mut AcquiredNodes<'_>,
        node_name: &str,
        user_space_params: &NormalizeParams,
        stroke_paint_source: Rc<UserSpacePaintSource>,
        fill_paint_source: Rc<UserSpacePaintSource>,
        node_bbox: BoundingBox,
    ) -> Result<SharedImageSurface, InternalRenderingError> {
        let session = self.session();

        // We try to convert each item in the filter_list to a FilterSpec.
        //
        // However, the spec mentions, "If the filter references a non-existent object or
        // the referenced object is not a filter element, then the whole filter chain is
        // ignored." - https://www.w3.org/TR/filter-effects/#FilterProperty
        //
        // So, run through the filter_list and collect into a Result<Vec<FilterSpec>>.
        // This will return an Err if any of the conversions failed.
        let filter_specs = filter
            .filter_list
            .iter()
            .map(|filter_value| {
                filter_value.to_filter_spec(
                    acquired_nodes,
                    user_space_params,
                    filter.current_color,
                    viewport,
                    session,
                    node_name,
                )
            })
            .collect::<Result<Vec<FilterSpec>, _>>();

        match filter_specs {
            Ok(specs) => {
                // Start with the surface_to_filter, and apply each filter spec in turn;
                // the final result is our return value.
                specs.iter().try_fold(surface_to_filter, |surface, spec| {
                    filters::render(
                        spec,
                        stroke_paint_source.clone(),
                        fill_paint_source.clone(),
                        surface,
                        acquired_nodes,
                        self,
                        *self.get_transform(),
                        node_bbox,
                    )
                })
            }

            Err(e) => {
                rsvg_log!(
                    self.session,
                    "not rendering filter list on node {} because it was in error: {}",
                    node_name,
                    e
                );
                // just return the original surface without filtering it
                Ok(surface_to_filter)
            }
        }
    }

    fn set_gradient(&mut self, gradient: &UserSpaceGradient) -> Result<(), InternalRenderingError> {
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

        g.set_matrix(ValidTransform::try_from(gradient.transform)?.into());
        g.set_extend(cairo::Extend::from(gradient.spread));

        for stop in &gradient.stops {
            let UnitInterval(stop_offset) = stop.offset;

            let rgba = color_to_rgba(&stop.color);

            g.add_color_stop_rgba(
                stop_offset,
                f64::from(rgba.red.unwrap_or(0)) / 255.0,
                f64::from(rgba.green.unwrap_or(0)) / 255.0,
                f64::from(rgba.blue.unwrap_or(0)) / 255.0,
                f64::from(rgba.alpha.unwrap_or(0.0)),
            );
        }

        Ok(self.cr.set_source(&g)?)
    }

    fn set_pattern(
        &mut self,
        pattern: &UserSpacePattern,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<bool, InternalRenderingError> {
        // Bail out early if the pattern has zero size, per the spec
        if approx_eq!(f64, pattern.width, 0.0) || approx_eq!(f64, pattern.height, 0.0) {
            return Ok(false);
        }

        // Bail out early if this pattern has a circular reference
        let pattern_node_acquired = match pattern.acquire_pattern_node(acquired_nodes) {
            Ok(n) => n,

            Err(AcquireError::CircularReference(ref node)) => {
                rsvg_log!(self.session, "circular reference in element {}", node);
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

        let transform = ValidTransform::try_from(caffine)?;
        cr_pattern.set_matrix(transform.into());

        // Draw everything

        {
            let mut pattern_draw_ctx = self.nested(cr_pattern);

            let pattern_viewport = Viewport {
                dpi: self.dpi,
                vbox: ViewBox::from(Rect::from_size(pattern.width, pattern.height)),
                transform: *transform,
            };

            pattern_draw_ctx
                .with_alpha(pattern.opacity, &mut |dc| {
                    let pattern_cascaded = CascadedValues::new_from_node(pattern_node);
                    let pattern_values = pattern_cascaded.get();

                    let elt = pattern_node.borrow_element();

                    let stacking_ctx = StackingContext::new(
                        self.session(),
                        acquired_nodes,
                        &elt,
                        Transform::identity(),
                        None,
                        pattern_values,
                    );

                    dc.with_discrete_layer(
                        &stacking_ctx,
                        acquired_nodes,
                        &pattern_viewport,
                        false,
                        &mut |an, dc| {
                            pattern_node.draw_children(
                                an,
                                &pattern_cascaded,
                                &pattern_viewport,
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
            pattern.set_matrix(ValidTransform::try_from(m)?.into());
            pattern.set_extend(cairo::Extend::Repeat);
            pattern.set_filter(cairo::Filter::Best);
            self.cr.set_source(&pattern)?;
        }

        Ok(true)
    }

    fn set_paint_source(
        &mut self,
        paint_source: &UserSpacePaintSource,
        acquired_nodes: &mut AcquiredNodes<'_>,
    ) -> Result<bool, InternalRenderingError> {
        match *paint_source {
            UserSpacePaintSource::Gradient(ref gradient, _c) => {
                self.set_gradient(gradient)?;
                Ok(true)
            }
            UserSpacePaintSource::Pattern(ref pattern, ref c) => {
                if self.set_pattern(pattern, acquired_nodes)? {
                    Ok(true)
                } else if let Some(c) = c {
                    set_source_color_on_cairo(&self.cr, c);
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            UserSpacePaintSource::SolidColor(ref c) => {
                set_source_color_on_cairo(&self.cr, c);
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
    ) -> Result<SharedImageSurface, InternalRenderingError> {
        let mut surface = ExclusiveImageSurface::new(width, height, SurfaceType::SRgb)?;

        surface.draw(&mut |cr| {
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
    ) -> Result<(), InternalRenderingError> {
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
    ) -> Result<(), InternalRenderingError> {
        let had_paint_server = self.set_paint_source(paint_source, acquired_nodes)?;
        if had_paint_server {
            cr.fill_preserve()?;
        }

        Ok(())
    }

    pub fn compute_path_extents(
        &self,
        path: &Path,
    ) -> Result<Option<Rect>, InternalRenderingError> {
        if path.is_empty() {
            return Ok(None);
        }

        let surface = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None)?;
        let cr = cairo::Context::new(&surface)?;

        path.to_cairo(&cr, false)?;
        let (x0, y0, x1, y1) = cr.path_extents()?;

        Ok(Some(Rect::new(x0, y0, x1, y1)))
    }

    pub fn draw_layer(
        &mut self,
        layer: &Layer,
        acquired_nodes: &mut AcquiredNodes<'_>,
        clipping: bool,
        viewport: &Viewport,
    ) -> Result<BoundingBox, InternalRenderingError> {
        match &layer.kind {
            LayerKind::Shape(shape) => self.draw_shape(
                shape,
                &layer.stacking_ctx,
                acquired_nodes,
                clipping,
                viewport,
            ),
            LayerKind::Text(text) => self.draw_text(
                text,
                &layer.stacking_ctx,
                acquired_nodes,
                clipping,
                viewport,
            ),
            LayerKind::Image(image) => self.draw_image(
                image,
                &layer.stacking_ctx,
                acquired_nodes,
                clipping,
                viewport,
            ),
        }
    }

    fn draw_shape(
        &mut self,
        shape: &Shape,
        stacking_ctx: &StackingContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        clipping: bool,
        viewport: &Viewport,
    ) -> Result<BoundingBox, InternalRenderingError> {
        if shape.extents.is_none() {
            return Ok(self.empty_bbox());
        }

        self.with_discrete_layer(
            stacking_ctx,
            acquired_nodes,
            viewport,
            clipping,
            &mut |an, dc| {
                let cr = dc.cr.clone();

                let transform = dc.get_transform_for_stacking_ctx(stacking_ctx, clipping)?;
                let mut path_helper =
                    PathHelper::new(&cr, transform, &shape.path, shape.stroke.line_cap);

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
                let bbox = compute_stroke_and_fill_box(
                    &cr,
                    &shape.stroke,
                    &shape.stroke_paint,
                    &dc.initial_viewport,
                )?;

                if shape.is_visible {
                    for &target in &shape.paint_order.targets {
                        // fill and stroke operations will preserve the path.
                        // markers operation will clear the path.
                        match target {
                            PaintTarget::Fill => {
                                path_helper.set()?;
                                dc.fill(&cr, an, &shape.fill_paint)?;
                            }

                            PaintTarget::Stroke => {
                                path_helper.set()?;
                                let backup_matrix = if shape.stroke.non_scaling {
                                    let matrix = cr.matrix();
                                    cr.set_matrix(
                                        ValidTransform::try_from(dc.initial_viewport.transform)?
                                            .into(),
                                    );
                                    Some(matrix)
                                } else {
                                    None
                                };
                                dc.stroke(&cr, an, &shape.stroke_paint)?;
                                if let Some(matrix) = backup_matrix {
                                    cr.set_matrix(matrix);
                                }
                            }

                            PaintTarget::Markers => {
                                path_helper.unset();
                                marker::render_markers_for_shape(
                                    shape, viewport, dc, an, clipping,
                                )?;
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
        image_rendering: ImageRendering,
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

        let interpolation = Interpolation::from(image_rendering);

        ptn.set_filter(cairo::Filter::from(interpolation));
        cr.set_source(&ptn)?;

        // Clip is needed due to extend being set to pad.
        clip_to_rectangle(&cr, &Rect::from_size(width, height));

        cr.paint()
    }

    fn draw_image(
        &mut self,
        image: &Image,
        stacking_ctx: &StackingContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        clipping: bool,
        viewport: &Viewport,
    ) -> Result<BoundingBox, InternalRenderingError> {
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
            ClipMode::ClipToViewport
        } else {
            ClipMode::NoClip
        };

        // The bounding box for <image> is decided by the values of the image's x, y, w, h
        // and not by the final computed image bounds.
        let bounds = self.empty_bbox().with_rect(image.rect);

        if image.is_visible {
            self.with_discrete_layer(
                stacking_ctx,
                acquired_nodes,
                viewport, // FIXME: should this be the push_new_viewport below?
                clipping,
                &mut |_an, dc| {
                    with_saved_cr(&dc.cr.clone(), || {
                        if let Some(_params) = dc.push_new_viewport(
                            viewport,
                            Some(vbox),
                            image.rect,
                            image.aspect,
                            clip_mode,
                        ) {
                            dc.paint_surface(
                                &image.surface,
                                image_width,
                                image_height,
                                image.image_rendering,
                            )?;
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
    ) -> Result<BoundingBox, InternalRenderingError> {
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
            let bbox = compute_stroke_and_fill_box(
                &self.cr,
                &span.stroke,
                &span.stroke_paint,
                &self.initial_viewport,
            )?;
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

    fn draw_text(
        &mut self,
        text: &Text,
        stacking_ctx: &StackingContext,
        acquired_nodes: &mut AcquiredNodes<'_>,
        clipping: bool,
        viewport: &Viewport,
    ) -> Result<BoundingBox, InternalRenderingError> {
        self.with_discrete_layer(
            stacking_ctx,
            acquired_nodes,
            viewport,
            clipping,
            &mut |an, dc| {
                let mut bbox = dc.empty_bbox();

                for span in &text.spans {
                    let span_bbox = dc.draw_text_span(span, an, clipping)?;
                    bbox.insert(&span_bbox);
                }

                Ok(bbox)
            },
        )
    }

    pub fn get_snapshot(
        &self,
        width: i32,
        height: i32,
    ) -> Result<SharedImageSurface, InternalRenderingError> {
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

                cr.set_matrix(ValidTransform::try_from(affines.for_snapshot)?.into());
                cr.set_source_surface(&draw.target(), 0.0, 0.0)?;
                cr.paint()?;
            }

            Ok(())
        })?;

        Ok(surface.share()?)
    }

    pub fn draw_node_to_surface(
        &mut self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        affine: Transform,
        width: i32,
        height: i32,
    ) -> Result<SharedImageSurface, InternalRenderingError> {
        let surface = cairo::ImageSurface::create(cairo::Format::ARgb32, width, height)?;

        let save_cr = self.cr.clone();

        {
            let cr = cairo::Context::new(&surface)?;
            cr.set_matrix(ValidTransform::try_from(affine)?.into());

            self.cr = cr;
            let viewport = Viewport {
                dpi: self.dpi,
                transform: affine,
                vbox: ViewBox::from(Rect::from_size(f64::from(width), f64::from(height))),
            };

            let _ = self.draw_node_from_stack(node, acquired_nodes, cascaded, &viewport, false)?;
        }

        self.cr = save_cr;

        Ok(SharedImageSurface::wrap(surface, SurfaceType::SRgb)?)
    }

    pub fn draw_node_from_stack(
        &mut self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        viewport: &Viewport,
        clipping: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        let stack_top = self.drawsub_stack.pop();

        let draw = if let Some(ref top) = stack_top {
            top == node
        } else {
            true
        };

        let res = if draw {
            node.draw(acquired_nodes, cascaded, viewport, self, clipping)
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
        viewport: &Viewport,
        fill_paint: Rc<PaintSource>,
        stroke_paint: Rc<PaintSource>,
    ) -> Result<BoundingBox, InternalRenderingError> {
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
                rsvg_log!(self.session, "circular reference in element {}", node);
                return Ok(self.empty_bbox());
            }

            _ => unreachable!(),
        };

        let acquired = match acquired_nodes.acquire(link) {
            Ok(acquired) => acquired,

            Err(AcquireError::CircularReference(node)) => {
                rsvg_log!(self.session, "circular reference in element {}", node);
                return Ok(self.empty_bbox());
            }

            Err(AcquireError::MaxReferencesExceeded) => {
                return Err(InternalRenderingError::LimitExceeded(
                    ImplementationLimit::TooManyReferencedElements,
                ));
            }

            Err(AcquireError::InvalidLinkType(_)) => unreachable!(),

            Err(AcquireError::LinkNotFound(node_id)) => {
                rsvg_log!(
                    self.session,
                    "element {} references nonexistent \"{}\"",
                    node,
                    node_id
                );
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

        self.cr
            .transform(ValidTransform::try_from(values.transform())?.into());

        let use_element = node.borrow_element();

        let defines_a_viewport = if is_element_of_type!(child, Symbol) {
            let symbol = borrow_element_as!(child, Symbol);
            Some((symbol.get_viewbox(), symbol.get_preserve_aspect_ratio()))
        } else if is_element_of_type!(child, Svg) {
            let svg = borrow_element_as!(child, Svg);
            Some((svg.get_viewbox(), svg.get_preserve_aspect_ratio()))
        } else {
            None
        };

        let res = if let Some((viewbox, preserve_aspect_ratio)) = defines_a_viewport {
            // <symbol> and <svg> define a viewport, as described in the specification:
            // https://www.w3.org/TR/SVG2/struct.html#UseElement
            // https://gitlab.gnome.org/GNOME/librsvg/-/issues/875#note_1482705

            let elt = child.borrow_element();

            let child_values = elt.get_computed_values();

            // FIXME: do we need to look at preserveAspectRatio.slice, like in draw_image()?
            let clip_mode = if !child_values.is_overflow() {
                ClipMode::ClipToViewport
            } else {
                ClipMode::NoClip
            };

            let stacking_ctx = StackingContext::new(
                self.session(),
                acquired_nodes,
                &use_element,
                Transform::identity(),
                None,
                values,
            );

            self.with_discrete_layer(
                &stacking_ctx,
                acquired_nodes,
                viewport, // FIXME: should this be the child_viewport from below?
                clipping,
                &mut |an, dc| {
                    if let Some(child_viewport) = dc.push_new_viewport(
                        viewport,
                        viewbox,
                        use_rect,
                        preserve_aspect_ratio,
                        clip_mode,
                    ) {
                        child.draw_children(
                            an,
                            &CascadedValues::new_from_values(
                                child,
                                values,
                                Some(fill_paint.clone()),
                                Some(stroke_paint.clone()),
                            ),
                            &child_viewport,
                            dc,
                            clipping,
                        )
                    } else {
                        Ok(dc.empty_bbox())
                    }
                },
            )
        } else {
            // otherwise the referenced node is not a <symbol>; process it generically

            let stacking_ctx = StackingContext::new(
                self.session(),
                acquired_nodes,
                &use_element,
                Transform::new_translate(use_rect.x0, use_rect.y0),
                None,
                values,
            );

            self.with_discrete_layer(
                &stacking_ctx,
                acquired_nodes,
                viewport,
                clipping,
                &mut |an, dc| {
                    child.draw(
                        an,
                        &CascadedValues::new_from_values(
                            child,
                            values,
                            Some(fill_paint.clone()),
                            Some(stroke_paint.clone()),
                        ),
                        viewport,
                        dc,
                        clipping,
                    )
                },
            )
        };

        self.cr.set_matrix(orig_transform.into());

        if let Ok(bbox) = res {
            let mut res_bbox = BoundingBox::new().with_transform(*orig_transform);
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

impl From<ImageRendering> for Interpolation {
    fn from(r: ImageRendering) -> Interpolation {
        match r {
            ImageRendering::Pixelated
            | ImageRendering::CrispEdges
            | ImageRendering::OptimizeSpeed => Interpolation::Nearest,

            ImageRendering::Smooth
            | ImageRendering::OptimizeQuality
            | ImageRendering::HighQuality
            | ImageRendering::Auto => Interpolation::Smooth,
        }
    }
}

/// Create a Pango context with a particular configuration.
pub fn create_pango_context(font_options: &FontOptions, transform: &Transform) -> pango::Context {
    let font_map = pangocairo::FontMap::default();
    let context = font_map.create_context();

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

pub fn set_source_color_on_cairo(cr: &cairo::Context, color: &cssparser::Color) {
    let rgba = color_to_rgba(color);

    cr.set_source_rgba(
        f64::from(rgba.red.unwrap_or(0)) / 255.0,
        f64::from(rgba.green.unwrap_or(0)) / 255.0,
        f64::from(rgba.blue.unwrap_or(0)) / 255.0,
        f64::from(rgba.alpha.unwrap_or(0.0)),
    );
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
) -> Result<Path, InternalRenderingError> {
    let surface = cairo::RecordingSurface::create(cairo::Content::ColorAlpha, None)?;
    let cr = cairo::Context::new(&surface)?;

    pango_layout_to_cairo(x, y, layout, gravity, &cr);

    let cairo_path = cr.copy_path()?;
    Ok(Path::from_cairo(cairo_path))
}

// https://www.w3.org/TR/css-masking-1/#ClipPathElement
fn element_can_be_used_inside_clip_path(element: &Element) -> bool {
    use ElementData::*;

    matches!(
        element.element_data,
        Circle(_)
            | Ellipse(_)
            | Line(_)
            | Path(_)
            | Polygon(_)
            | Polyline(_)
            | Rect(_)
            | Text(_)
            | Use(_)
    )
}

// https://www.w3.org/TR/css-masking-1/#ClipPathElement
fn element_can_be_used_inside_use_inside_clip_path(element: &Element) -> bool {
    use ElementData::*;

    matches!(
        element.element_data,
        Circle(_) | Ellipse(_) | Line(_) | Path(_) | Polygon(_) | Polyline(_) | Rect(_) | Text(_)
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
    stroke_paint_source: &UserSpacePaintSource,
    initial_viewport: &Viewport,
) -> Result<PathExtents, InternalRenderingError> {
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
    let fill_extents = if x0 != 0.0 || y0 != 0.0 || x1 != 0.0 || y1 != 0.0 {
        Some(Rect::new(x0, y0, x1, y1))
    } else {
        None
    };

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
        && !matches!(stroke_paint_source, UserSpacePaintSource::None)
    {
        let backup_matrix = if stroke.non_scaling {
            let matrix = cr.matrix();
            cr.set_matrix(ValidTransform::try_from(initial_viewport.transform)?.into());
            Some(matrix)
        } else {
            None
        };
        let (x0, y0, x1, y1) = cr.stroke_extents()?;
        if let Some(matrix) = backup_matrix {
            cr.set_matrix(matrix);
        }
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
    stroke_paint_source: &UserSpacePaintSource,
    initial_viewport: &Viewport,
) -> Result<BoundingBox, InternalRenderingError> {
    let extents =
        compute_stroke_and_fill_extents(cr, stroke, stroke_paint_source, initial_viewport)?;

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
    let regex = {
        static REGEX: OnceLock<Regex> = OnceLock::new();
        REGEX.get_or_init(|| Regex::new(r"['\\]").unwrap())
    };

    regex.replace_all(value, |caps: &Captures<'_>| {
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
        Self::new_unchecked(m.xx(), m.yx(), m.xy(), m.yy(), m.x0(), m.y0())
    }
}

impl From<ValidTransform> for cairo::Matrix {
    #[inline]
    fn from(t: ValidTransform) -> cairo::Matrix {
        cairo::Matrix::new(t.xx, t.yx, t.xy, t.yy, t.x0, t.y0)
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
    ) -> Result<(), InternalRenderingError> {
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
