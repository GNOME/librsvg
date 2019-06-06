use cairo;
use cairo::prelude::*;
use cairo_sys;
use glib::translate::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::allowed_url::Fragment;
use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::clip_path::{ClipPathUnits, NodeClipPath};
use crate::coord_units::CoordUnits;
use crate::dpi::Dpi;
use crate::error::RenderingError;
use crate::filters;
use crate::gradient::NodeGradient;
use crate::length::Dasharray;
use crate::mask::NodeMask;
use crate::node::{CascadedValues, NodeType, RsvgNode};
use crate::paint_server::{PaintServer, PaintSource};
use crate::pattern::NodePattern;
use crate::properties::ComputedValues;
use crate::property_defs::{
    ClipRule,
    FillRule,
    ShapeRendering,
    StrokeDasharray,
    StrokeLinecap,
    StrokeLinejoin,
};
use crate::rect::RectangleExt;
use crate::surface_utils::shared_surface::SharedImageSurface;
use crate::svg::Svg;
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

pub struct DrawingCtx {
    svg: Rc<Svg>,

    initial_affine: cairo::Matrix,

    rect: cairo::Rectangle,
    dpi: Dpi,

    /// This is a mitigation for the security-related bug
    /// https://gitlab.gnome.org/GNOME/librsvg/issues/323 - imagine
    /// the XML [billion laughs attack], but done by creating deeply
    /// nested groups of `<use>` elements.  The first one references
    /// the second one ten times, the second one references the third
    /// one ten times, and so on.  In the file given, this causes
    /// 10^17 objects to be rendered.  While this does not exhaust
    /// memory, it would take a really long time.
    ///
    /// [billion laughs attack]: https://bitbucket.org/tiran/defusedxml
    num_elements_rendered_through_use: usize,

    cr_stack: Vec<cairo::Context>,
    cr: cairo::Context,

    view_box_stack: Rc<RefCell<Vec<ViewBox>>>,

    bbox: BoundingBox,

    drawsub_stack: Vec<RsvgNode>,

    acquired_nodes: AcquiredNodes,

    measuring: bool,
    testing: bool,
}

impl DrawingCtx {
    pub fn new(
        svg: Rc<Svg>,
        node: Option<&RsvgNode>,
        cr: &cairo::Context,
        viewport: &cairo::Rectangle,
        dpi: Dpi,
        measuring: bool,
        testing: bool,
    ) -> DrawingCtx {
        let initial_affine = cr.get_matrix();

        // This is more or less a hack to make measuring geometries possible,
        // while the code gets refactored not to need special cases for that.

        let (rect, vbox) = if measuring {
            (
                cairo::Rectangle::new(0.0, 0.0, 1.0, 1.0),
                ViewBox::new(0.0, 0.0, 1.0, 1.0),
            )
        } else {
            let rect = *viewport;

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
            let vbox = ViewBox {
                x: 0.0,
                y: 0.0,
                width: viewport.width,
                height: viewport.height,
            };

            (rect, vbox)
        };

        let mut view_box_stack = Vec::new();
        view_box_stack.push(vbox);

        let acquired_nodes = AcquiredNodes::new(svg.clone());

        let mut draw_ctx = DrawingCtx {
            svg,
            initial_affine,
            rect,
            dpi,
            num_elements_rendered_through_use: 0,
            cr_stack: Vec::new(),
            cr: cr.clone(),
            view_box_stack: Rc::new(RefCell::new(view_box_stack)),
            bbox: BoundingBox::new(&initial_affine),
            drawsub_stack: Vec::new(),
            acquired_nodes,
            measuring,
            testing,
        };

        if let Some(node) = node {
            draw_ctx.add_node_and_ancestors_to_stack(node);
        }

        draw_ctx
    }

    pub fn toplevel_viewport(&self) -> cairo::Rectangle {
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

    // FIXME: Usage of this function is more less a hack... The caller
    // manually saves and then restore the draw_ctx.cr.
    // It would be better to have an explicit push/pop for the cairo_t, or
    // pushing a temporary surface, or something that does not involve
    // monkeypatching the cr directly.
    pub fn set_cairo_context(&mut self, cr: &cairo::Context) {
        self.cr = cr.clone();
    }

    // Temporary hack while we unify surface/cr/affine creation
    pub fn push_cairo_context(&mut self, cr: cairo::Context) {
        self.cr_stack.push(self.cr.clone());
        self.cr = cr;
    }

    // Temporary hack while we unify surface/cr/affine creation
    pub fn pop_cairo_context(&mut self) {
        self.cr = self.cr_stack.pop().unwrap();
    }

    fn size_for_temporary_surface(&self) -> (i32, i32) {
        // We need a size in whole pixels, so use ceil() to ensure the whole viewport fits
        // into the temporary surface.
        let width = self.rect.width.ceil() as i32;
        let height = self.rect.height.ceil() as i32;

        (width, height)
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

        let surface = cairo::Surface::create_similar(surface, cairo::Content::ColorAlpha, w, h);

        // FIXME: cairo-rs should return a Result from create_similar()!
        // Since it doesn't, we need to check its status by hand...

        let status = surface.status();
        if status == cairo::Status::Success {
            Ok(surface)
        } else {
            Err(RenderingError::Cairo(status))
        }
    }

    /// Gets the viewport that was last pushed with `push_view_box()`.
    pub fn get_view_params(&self) -> ViewParams {
        let view_box_stack = self.view_box_stack.borrow();
        let last = view_box_stack.len() - 1;
        let stack_top = &view_box_stack[last];

        ViewParams {
            dpi_x: self.dpi.x(),
            dpi_y: self.dpi.y(),
            view_box_width: stack_top.width,
            view_box_height: stack_top.height,
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
            .push(ViewBox::new(0.0, 0.0, width, height));

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
        viewport: &cairo::Rectangle,
        preserve_aspect_ratio: AspectRatio,
        clip_mode: Option<ClipMode>,
    ) -> Option<ViewParams> {
        if let Some(ref clip) = clip_mode {
            if *clip == ClipMode::ClipToViewport {
                self.clip(viewport.x, viewport.y, viewport.width, viewport.height);
            }
        }

        preserve_aspect_ratio
            .viewport_to_viewbox_transform(vbox, viewport)
            .and_then(|matrix| {
                self.cr.transform(matrix);

                if let Some(vbox) = vbox {
                    if let Some(ref clip) = clip_mode {
                        if *clip == ClipMode::ClipToVbox {
                            self.clip(vbox.x, vbox.y, vbox.width, vbox.height);
                        }
                    }

                    Some(self.push_view_box(vbox.width, vbox.height))
                } else {
                    Some(self.get_view_params())
                }
            })
    }

    pub fn insert_bbox(&mut self, bbox: &BoundingBox) {
        self.bbox.insert(bbox);
    }

    pub fn get_bbox(&self) -> &BoundingBox {
        &self.bbox
    }

    pub fn acquired_nodes(&self) -> &AcquiredNodes {
        &self.acquired_nodes
    }

    // Returns (clip_in_user_space, clip_in_object_space), both Option<RsvgNode>
    fn get_clip_in_user_and_object_space(
        &mut self,
        clip_uri: Option<&Fragment>,
    ) -> (Option<RsvgNode>, Option<RsvgNode>) {
        if let Some(clip_node) = self
            .acquired_nodes
            .get_node_of_type(clip_uri, NodeType::ClipPath)
        {
            let clip_node = clip_node.get().clone();

            let ClipPathUnits(units) = clip_node.borrow().get_impl::<NodeClipPath>().get_units();

            if units == CoordUnits::UserSpaceOnUse {
                (Some(clip_node), None)
            } else {
                assert!(units == CoordUnits::ObjectBoundingBox);
                (None, Some(clip_node))
            }
        } else {
            (None, None)
        }
    }

    fn clip_to_node(&mut self, clip_node: &Option<RsvgNode>) -> Result<(), RenderingError> {
        if let Some(node) = clip_node {
            let orig_bbox = self.bbox;

            let clip_path = node.borrow().get_impl::<NodeClipPath>();
            let res = clip_path.to_cairo_context(&node, self, &orig_bbox);

            // FIXME: this is an EPIC HACK to keep the clipping context from
            // accumulating bounding boxes.  We'll remove this later, when we
            // are able to extract bounding boxes from outside the
            // general drawing loop.
            self.bbox = orig_bbox;

            res
        } else {
            Ok(())
        }
    }

    pub fn with_discrete_layer(
        &mut self,
        node: &RsvgNode,
        values: &ComputedValues,
        clipping: bool,
        draw_fn: &mut FnMut(&mut DrawingCtx) -> Result<(), RenderingError>,
    ) -> Result<(), RenderingError> {
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

                let affine_at_start = dc.cr.get_matrix();

                let (clip_in_user_space, clip_in_object_space) =
                    dc.get_clip_in_user_and_object_space(clip_uri);

                dc.clip_to_node(&clip_in_user_space)?;

                let needs_temporary_surface = !(opacity == 1.0
                    && filter.is_none()
                    && mask.is_none()
                    && clip_in_object_space.is_none());

                if needs_temporary_surface {
                    // Compute our assortment of affines

                    let affines = CompositingAffines::new(
                        affine_at_start,
                        dc.initial_affine_with_offset(),
                        dc.cr_stack.len(),
                    );

                    // Create temporary surface and its cr

                    let cr = if filter.is_some() {
                        cairo::Context::new(&dc.create_surface_for_toplevel_viewport()?)
                    } else {
                        cairo::Context::new(
                            &dc.create_similar_surface_for_toplevel_viewport(&dc.cr.get_target())?,
                        )
                    };

                    cr.set_matrix(affines.for_temporary_surface);

                    dc.push_cairo_context(cr);

                    // Create temporary bbox with the cr's affine

                    let prev_bbox = dc.bbox;

                    dc.bbox = BoundingBox::new(&affines.for_temporary_surface);

                    // Draw!

                    let mut res = draw_fn(dc);

                    // Filter

                    let source_surface = if let Some(filter_uri) = filter {
                        let child_surface = cairo::ImageSurface::from(dc.cr.get_target()).unwrap();
                        let img_surface =
                            dc.run_filter(filter_uri, node, values, &child_surface, dc.bbox)?;
                        // turn into a Surface
                        img_surface.as_ref().clone()
                    } else {
                        dc.cr.get_target()
                    };

                    dc.pop_cairo_context();

                    // Set temporary surface as source

                    dc.cr.set_matrix(affines.compositing);
                    dc.cr.set_source_surface(&source_surface, 0.0, 0.0);

                    // Clip

                    dc.cr.set_matrix(affines.outside_temporary_surface);
                    dc.clip_to_node(&clip_in_object_space)?;

                    // Mask

                    if let Some(mask) = mask {
                        if let Some(acquired) = dc
                            .acquired_nodes
                            .get_node_of_type(Some(mask), NodeType::Mask)
                        {
                            let mask_node = acquired.get();

                            res = res.and_then(|_| {
                                let bbox = dc.bbox;
                                mask_node
                                    .borrow()
                                    .get_impl::<NodeMask>()
                                    .generate_cairo_mask(&mask_node, &affines, dc, &bbox)
                            });
                        } else {
                            rsvg_log!("element {} references nonexistent mask \"{}\"", node, mask);
                        }
                    } else {
                        // No mask, so composite the temporary surface

                        dc.cr.set_matrix(affines.compositing);

                        if opacity < 1.0 {
                            dc.cr.paint_with_alpha(opacity);
                        } else {
                            dc.cr.paint();
                        }
                    }

                    dc.cr.set_matrix(affine_at_start);

                    let bbox = dc.bbox;
                    dc.bbox = prev_bbox;
                    dc.bbox.insert(&bbox);

                    res
                } else {
                    draw_fn(dc)
                }
            })
        }
    }

    fn initial_affine_with_offset(&self) -> cairo::Matrix {
        let mut initial_with_offset = self.initial_affine;
        initial_with_offset.translate(self.rect.x, self.rect.y);
        initial_with_offset
    }

    /// Saves the current Cairo matrix, runs the draw_fn, and restores the matrix
    ///
    /// This is slightly cheaper than a `cr.save()` / `cr.restore()`
    /// pair, but more importantly, it does not reset the whole
    /// graphics state, i.e. it leaves a clipping path in place if it
    /// was set by the `draw_fn`.
    pub fn with_saved_matrix(
        &mut self,
        draw_fn: &mut FnMut(&mut DrawingCtx) -> Result<(), RenderingError>,
    ) -> Result<(), RenderingError> {
        let matrix = self.cr.get_matrix();
        let res = draw_fn(self);
        self.cr.set_matrix(matrix);
        res
    }

    /// Saves the current Cairo context, runs the draw_fn, and restores the context
    pub fn with_saved_cr(
        &mut self,
        draw_fn: &mut FnMut(&mut DrawingCtx) -> Result<(), RenderingError>,
    ) -> Result<(), RenderingError> {
        self.cr.save();
        let res = draw_fn(self);
        self.cr.restore();
        res
    }

    fn run_filter(
        &mut self,
        filter_uri: &Fragment,
        node: &RsvgNode,
        values: &ComputedValues,
        child_surface: &cairo::ImageSurface,
        node_bbox: BoundingBox,
    ) -> Result<cairo::ImageSurface, RenderingError> {
        match self
            .acquired_nodes
            .get_node_of_type(Some(filter_uri), NodeType::Filter)
        {
            Some(acquired) => {
                let filter_node = acquired.get();

                if !filter_node.borrow().is_in_error() {
                    // FIXME: deal with out of memory here
                    filters::render(&filter_node, values, child_surface, self, node_bbox)
                } else {
                    Ok(child_surface.clone())
                }
            }

            None => {
                rsvg_log!(
                    "element {} will not be rendered since its filter \"{}\" was not found",
                    node,
                    filter_uri,
                );

                // Non-existing filters must act as null filters (that is, an
                // empty surface is returned).
                Ok(cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    child_surface.get_width(),
                    child_surface.get_height(),
                )?)
            }
        }
    }

    fn set_color(
        &self,
        color: &cssparser::Color,
        opacity: &UnitInterval,
        current_color: &cssparser::RGBA,
    ) {
        let rgba = match *color {
            cssparser::Color::RGBA(ref rgba) => rgba,
            cssparser::Color::CurrentColor => current_color,
        };

        let &UnitInterval(o) = opacity;
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
        opacity: &UnitInterval,
        bbox: &BoundingBox,
        current_color: &cssparser::RGBA,
    ) -> Result<bool, RenderingError> {
        match *ps {
            PaintServer::Iri {
                ref iri,
                ref alternate,
            } => {
                let mut had_paint_server = false;

                if let Some(acquired) = self.acquired_nodes.get_node(iri) {
                    let node = acquired.get();

                    had_paint_server = match node.borrow().get_type() {
                        NodeType::LinearGradient | NodeType::RadialGradient => node
                            .borrow()
                            .get_impl::<NodeGradient>()
                            .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                        NodeType::Pattern => node
                            .borrow()
                            .get_impl::<NodePattern>()
                            .resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)?,
                        _ => false,
                    }
                }

                if !had_paint_server && alternate.is_some() {
                    self.set_color(alternate.as_ref().unwrap(), opacity, current_color);
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
                self.set_color(&color, opacity, current_color);
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
    ) -> Result<(), RenderingError> {
        cr.set_antialias(cairo::Antialias::from(values.shape_rendering));

        self.setup_cr_for_stroke(cr, values);

        // Update the bbox in the rendering context.  Below, we actually set the
        // fill/stroke patterns on the cairo_t.  That process requires the
        // rendering context to have an updated bbox; for example, for the
        // coordinate system in patterns.
        let bbox = compute_stroke_and_fill_box(cr, values);
        self.insert_bbox(&bbox);

        let current_color = &values.color.0;

        let fill_opacity = &values.fill_opacity.0;

        let res = self
            .set_source_paint_server(&values.fill.0, fill_opacity, &bbox, current_color)
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
                let stroke_opacity = values.stroke_opacity.0;

                self.set_source_paint_server(
                    &values.stroke.0,
                    &stroke_opacity,
                    &bbox,
                    &current_color,
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

        res
    }

    pub fn clip(&self, x: f64, y: f64, w: f64, h: f64) {
        let cr = self.get_cairo_context();

        cr.rectangle(x, y, w, h);
        cr.clip();
    }

    pub fn get_snapshot(&self, surface: &cairo::ImageSurface) {
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
        let cr = cairo::Context::new(&surface);
        for (depth, draw) in self.cr_stack.iter().enumerate() {
            let affines = CompositingAffines::new(
                draw.get_matrix(),
                self.initial_affine_with_offset(),
                depth,
            );

            cr.set_matrix(affines.for_snapshot);
            cr.set_source_surface(&draw.get_target(), 0.0, 0.0);
            cr.paint();
        }
    }

    pub fn lookup_image(&self, href: &str) -> Result<SharedImageSurface, RenderingError> {
        self.svg
            .lookup_image(href)
            .map_err(|_| RenderingError::InvalidHref)
    }

    pub fn draw_node_on_surface(
        &mut self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        surface: &cairo::ImageSurface,
        width: f64,
        height: f64,
    ) -> Result<(), RenderingError> {
        let save_cr = self.cr.clone();
        let save_rect = self.rect;
        let save_affine = self.get_cairo_context().get_matrix();

        let cr = cairo::Context::new(surface);
        cr.set_matrix(save_affine);

        self.cr = cr;
        self.rect.x = 0.0;
        self.rect.y = 0.0;
        self.rect.width = width;
        self.rect.height = height;

        let res = self.draw_node_from_stack(cascaded, node, false);

        self.cr = save_cr;
        self.rect = save_rect;

        res
    }

    pub fn draw_node_from_stack(
        &mut self,
        cascaded: &CascadedValues<'_>,
        node: &RsvgNode,
        clipping: bool,
    ) -> Result<(), RenderingError> {
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
            Ok(())
        };

        if let Some(top) = stack_top {
            self.drawsub_stack.push(top);
        }

        res.and_then(|_| self.check_limits())
    }

    pub fn add_node_and_ancestors_to_stack(&mut self, node: &RsvgNode) {
        self.drawsub_stack.push(node.clone());
        if let Some(ref parent) = node.parent() {
            self.add_node_and_ancestors_to_stack(parent);
        }
    }

    pub fn increase_num_elements_rendered_through_use(&mut self, n: usize) {
        self.num_elements_rendered_through_use += n;
    }

    fn check_limits(&self) -> Result<(), RenderingError> {
        if self.num_elements_rendered_through_use > 500_000 {
            Err(RenderingError::InstancingLimit)
        } else {
            Ok(())
        }
    }
}

pub struct CompositingAffines {
    pub outside_temporary_surface: cairo::Matrix,
    pub initial: cairo::Matrix,
    pub for_temporary_surface: cairo::Matrix,
    pub compositing: cairo::Matrix,
    pub for_snapshot: cairo::Matrix,
}

impl CompositingAffines {
    fn new(
        current: cairo::Matrix,
        initial: cairo::Matrix,
        cr_stack_depth: usize,
    ) -> CompositingAffines {
        let is_topmost_temporary_surface = cr_stack_depth == 0;

        let initial_inverse = initial.try_invert().unwrap();

        let outside_temporary_surface = if is_topmost_temporary_surface {
            current
        } else {
            cairo::Matrix::multiply(&current, &initial_inverse)
        };

        let for_temporary_surface = if is_topmost_temporary_surface {
            let untransformed = cairo::Matrix::multiply(&current, &initial_inverse);
            untransformed
        } else {
            current
        };

        let compositing = if is_topmost_temporary_surface {
            initial
        } else {
            cairo::Matrix::identity()
        };

        // This is the inverse of "compositing"; we do it this way
        // instead of inverting that one to preserve accuracy.
        let for_snapshot = if is_topmost_temporary_surface {
            initial_inverse
        } else {
            cairo::Matrix::identity()
        };

        CompositingAffines {
            outside_temporary_surface,
            initial,
            for_temporary_surface,
            compositing,
            for_snapshot,
        }
    }
}

fn compute_stroke_and_fill_box(cr: &cairo::Context, values: &ComputedValues) -> BoundingBox {
    let affine = cr.get_matrix();

    let mut bbox = BoundingBox::new(&affine);

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

    let fb = BoundingBox::new(&affine).with_ink_extents(cr.fill_extents());
    bbox.insert(&fb);

    // Bounding box for stroke

    if values.stroke.0 != PaintServer::None {
        let sb = BoundingBox::new(&affine).with_ink_extents(cr.stroke_extents());
        bbox.insert(&sb);
    }

    // objectBoundingBox

    let ob = BoundingBox::new(&affine).with_extents(path_extents(cr));
    bbox.insert(&ob);

    // restore tolerance

    cr.set_tolerance(backup_tolerance);

    bbox
}

// remove this binding once cairo-rs has Context::path_extents()
fn path_extents(cr: &cairo::Context) -> (f64, f64, f64, f64) {
    let mut x1: f64 = 0.0;
    let mut y1: f64 = 0.0;
    let mut x2: f64 = 0.0;
    let mut y2: f64 = 0.0;

    unsafe {
        cairo_sys::cairo_path_extents(cr.to_glib_none().0, &mut x1, &mut y1, &mut x2, &mut y2);
    }
    (x1, y1, x2, y2)
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

#[derive(Default, Clone, Copy, Debug, PartialEq)]
#[repr(C)]
pub struct RsvgRectangle {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl From<cairo::Rectangle> for RsvgRectangle {
    fn from(r: cairo::Rectangle) -> RsvgRectangle {
        RsvgRectangle {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

impl From<ViewBox> for RsvgRectangle {
    fn from(vb: ViewBox) -> RsvgRectangle {
        RsvgRectangle {
            x: vb.x,
            y: vb.y,
            width: vb.width,
            height: vb.height,
        }
    }
}

impl From<RsvgRectangle> for cairo::Rectangle {
    fn from(r: RsvgRectangle) -> cairo::Rectangle {
        cairo::Rectangle {
            x: r.x,
            y: r.y,
            width: r.width,
            height: r.height,
        }
    }
}

pub struct AcquiredNode(Rc<RefCell<NodeStack>>, RsvgNode);

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        let mut stack = self.0.borrow_mut();
        let last = stack.pop().unwrap();
        assert!(last == self.1);
    }
}

impl AcquiredNode {
    pub fn get(&self) -> &RsvgNode {
        &self.1
    }
}

pub struct AcquiredNodes {
    svg: Rc<Svg>,
    node_stack: Rc<RefCell<NodeStack>>,
}

impl AcquiredNodes {
    pub fn new(svg: Rc<Svg>) -> AcquiredNodes {
        AcquiredNodes {
            svg,
            node_stack: Rc::new(RefCell::new(NodeStack::new())),
        }
    }

    // Use this function when looking up urls to other nodes. This function
    // does proper recursion checking and thereby avoids infinite loops.
    //
    // Nodes acquired by this function must be released in reverse
    // acquiring order.
    //
    // Note that if you acquire a node, you have to release it before trying to
    // acquire it again.  If you acquire a node "#foo" and don't release it before
    // trying to acquire "foo" again, you will obtain a %NULL the second time.
    pub fn get_node(&self, fragment: &Fragment) -> Option<AcquiredNode> {
        if let Ok(node) = self.svg.lookup(fragment) {
            if !self.node_stack.borrow().contains(&node) {
                self.node_stack.borrow_mut().push(&node);
                let acq = AcquiredNode(self.node_stack.clone(), node.clone());
                return Some(acq);
            }
        }

        None
    }

    // Use this function when looking up urls to other nodes, and when you expect
    // the node to be of a particular type. This function does proper recursion
    // checking and thereby avoids infinite loops.
    //
    // Malformed SVGs, for example, may reference a marker by its IRI, but
    // the object referenced by the IRI is not a marker.
    //
    // Note that if you acquire a node, you have to release it before trying to
    // acquire it again.  If you acquire a node "#foo" and don't release it before
    // trying to acquire "foo" again, you will obtain a None the second time.
    //
    // For convenience, this function will return None if url is None.

    // FIXME: return a Result<AcquiredNode, RenderingError::InvalidReference>
    pub fn get_node_of_type(
        &self,
        fragment: Option<&Fragment>,
        node_type: NodeType,
    ) -> Option<AcquiredNode> {
        fragment
            .and_then(move |fragment| self.get_node(fragment))
            .and_then(|acquired| {
                if acquired.get().borrow().get_type() == node_type {
                    Some(acquired)
                } else {
                    None
                }
            })
    }
}

/// Keeps a stack of nodes and can check if a certain node is contained in the stack
///
/// Sometimes parts of the code cannot plainly use the implicit stack of acquired
/// nodes as maintained by DrawingCtx::acquired_nodes, and they must keep their
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
