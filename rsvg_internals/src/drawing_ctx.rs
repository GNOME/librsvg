use cairo;
use cairo::MatrixTrait;
use cairo_sys;
use glib::translate::*;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

use allowed_url::Fragment;
use bbox::BoundingBox;
use clip_path::{ClipPathUnits, NodeClipPath};
use coord_units::CoordUnits;
use dpi::Dpi;
use error::RenderingError;
use filters;
use gradient::NodeGradient;
use length::Dasharray;
use mask::NodeMask;
use node::{CascadedValues, NodeType, RsvgNode};
use paint_server::{PaintServer, PaintSource};
use pattern::NodePattern;
use properties::{
    ClipRule,
    ComputedValues,
    EnableBackground,
    FillRule,
    ShapeRendering,
    StrokeDasharray,
    StrokeLinecap,
    StrokeLinejoin,
};
use rect::RectangleExt;
use surface_utils::shared_surface::SharedImageSurface;
use svg::Svg;
use unit_interval::UnitInterval;
use viewbox::ViewBox;

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
    dpi_x: f64,
    dpi_y: f64,
    view_box_width: f64,
    view_box_height: f64,
    view_box_stack: Option<Weak<RefCell<Vec<ViewBox>>>>,
}

impl ViewParams {
    #[cfg(test)]
    pub fn new(dpi_x: f64, dpi_y: f64, view_box_width: f64, view_box_height: f64) -> ViewParams {
        ViewParams {
            dpi_x,
            dpi_y,
            view_box_width,
            view_box_height,
            view_box_stack: None,
        }
    }

    pub fn dpi_x(&self) -> f64 {
        self.dpi_x
    }

    pub fn dpi_y(&self) -> f64 {
        self.dpi_y
    }

    pub fn view_box_width(&self) -> f64 {
        self.view_box_width
    }

    pub fn view_box_height(&self) -> f64 {
        self.view_box_height
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

pub struct DrawingCtx {
    svg: Rc<Svg>,

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
    initial_cr: cairo::Context,

    surfaces_stack: Vec<cairo::ImageSurface>,

    view_box_stack: Rc<RefCell<Vec<ViewBox>>>,

    bbox: BoundingBox,
    bbox_stack: Vec<BoundingBox>,

    drawsub_stack: Vec<RsvgNode>,

    acquired_nodes: Rc<RefCell<Vec<RsvgNode>>>,

    testing: bool,
}

impl DrawingCtx {
    pub fn new(
        svg: Rc<Svg>,
        cr: &cairo::Context,
        width: f64,
        height: f64,
        vb_width: f64,
        vb_height: f64,
        dpi: Dpi,
        testing: bool,
    ) -> DrawingCtx {
        let mut affine = cr.get_matrix();
        let rect = cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
        .transform(&affine)
        .outer();

        // scale according to size set by size_func callback
        let mut scale = cairo::Matrix::identity();
        scale.scale(width / vb_width, height / vb_height);
        affine = cairo::Matrix::multiply(&affine, &scale);

        // adjust transform so that the corner of the
        // bounding box above is at (0,0)
        affine.x0 -= rect.x;
        affine.y0 -= rect.y;
        cr.set_matrix(affine);

        let mut view_box_stack = Vec::new();
        view_box_stack.push(ViewBox::new(0.0, 0.0, vb_width, vb_height));

        DrawingCtx {
            svg: svg.clone(),
            rect,
            dpi,
            num_elements_rendered_through_use: 0,
            cr_stack: Vec::new(),
            cr: cr.clone(),
            initial_cr: cr.clone(),
            surfaces_stack: Vec::new(),
            view_box_stack: Rc::new(RefCell::new(view_box_stack)),
            bbox: BoundingBox::new(&affine),
            bbox_stack: Vec::new(),
            drawsub_stack: Vec::new(),
            acquired_nodes: Rc::new(RefCell::new(Vec::new())),
            testing,
        }
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

    pub fn get_width(&self) -> f64 {
        self.rect.width
    }

    pub fn get_height(&self) -> f64 {
        self.rect.height
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

    pub fn insert_bbox(&mut self, bbox: &BoundingBox) {
        self.bbox.insert(bbox);
    }

    pub fn set_bbox(&mut self, bbox: &BoundingBox) {
        self.bbox = *bbox;
    }

    pub fn get_bbox(&self) -> &BoundingBox {
        &self.bbox
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
    pub fn get_acquired_node(&mut self, fragment: &Fragment) -> Option<AcquiredNode> {
        if let Ok(node) = self.svg.lookup(fragment) {
            if !self.acquired_nodes_contains(&node) {
                self.acquired_nodes.borrow_mut().push(node.clone());
                let acq = AcquiredNode(self.acquired_nodes.clone(), node.clone());
                return Some(acq);
            }
        }

        None
    }

    fn acquired_nodes_contains(&self, node: &RsvgNode) -> bool {
        self.acquired_nodes
            .borrow()
            .iter()
            .find(|n| Rc::ptr_eq(n, node))
            .is_some()
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
    pub fn get_acquired_node_of_type(
        &mut self,
        fragment: Option<&Fragment>,
        node_type: NodeType,
    ) -> Option<AcquiredNode> {
        fragment
            .and_then(move |fragment| self.get_acquired_node(fragment))
            .and_then(|acquired| {
                if acquired.get().get_type() == node_type {
                    Some(acquired)
                } else {
                    None
                }
            })
    }

    fn is_cairo_context_nested(&self, cr: &cairo::Context) -> bool {
        cr.to_raw_none() != self.initial_cr.to_raw_none()
    }

    fn get_offset(&self) -> (f64, f64) {
        if self.is_cairo_context_nested(&self.get_cairo_context()) {
            (0.0, 0.0)
        } else {
            (self.rect.x, self.rect.y)
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
            let original_cr = self.cr.clone();
            original_cr.save();

            let clip_uri = values.clip_path.0.get();
            let mask = values.mask.0.get();

            // The `filter` property does not apply to masks.
            let filter = if node.get_type() == NodeType::Mask {
                None
            } else {
                values.filter.0.get()
            };

            let UnitInterval(opacity) = values.opacity.0;
            let enable_background = values.enable_background;

            let affine = original_cr.get_matrix();

            let (clip_in_user_space, clip_in_object_space) = {
                if let Some(clip_node) =
                    self.get_acquired_node_of_type(clip_uri, NodeType::ClipPath)
                {
                    let clip_node = clip_node.get().clone();

                    let ClipPathUnits(units) =
                        clip_node.with_impl(|clip_path: &NodeClipPath| clip_path.get_units());

                    if units == CoordUnits::UserSpaceOnUse {
                        (Some(clip_node), None)
                    } else {
                        assert!(units == CoordUnits::ObjectBoundingBox);
                        (None, Some(clip_node))
                    }
                } else {
                    (None, None)
                }
            };

            if let Some(clip_node) = clip_in_user_space {
                clip_node
                    .with_impl(|clip_path: &NodeClipPath| {
                        clip_path.to_cairo_context(&clip_node, &affine, self)
                    })
                    .map_err(|e| {
                        original_cr.restore();
                        e
                    })?;
            }

            let needs_temporary_surface = !(opacity == 1.0
                && filter.is_none()
                && mask.is_none()
                && clip_in_object_space.is_none()
                && enable_background == EnableBackground::Accumulate);

            if needs_temporary_surface {
                let surface = cairo::ImageSurface::create(
                    cairo::Format::ARgb32,
                    self.rect.width as i32,
                    self.rect.height as i32,
                )?;

                if filter.is_some() {
                    self.surfaces_stack.push(surface.clone());
                }

                let cr = cairo::Context::new(&surface);
                cr.set_matrix(affine);

                self.cr_stack.push(self.cr.clone());
                self.cr = cr.clone();

                self.bbox_stack.push(self.bbox);
                self.bbox = BoundingBox::new(&affine);
            }

            let mut res = draw_fn(self);

            if needs_temporary_surface {
                let child_surface = cairo::ImageSurface::from(self.cr.get_target()).unwrap();

                let filter_result_surface = if let Some(filter_uri) = filter {
                    self.run_filter(filter_uri, node, values, &child_surface)?
                } else {
                    child_surface
                };

                self.cr = self.cr_stack.pop().unwrap();

                let (xofs, yofs) = self.get_offset();

                original_cr.identity_matrix();
                original_cr.set_source_surface(&filter_result_surface, xofs, yofs);

                if let Some(clip_node) = clip_in_object_space {
                    clip_node
                        .with_impl(|clip_path: &NodeClipPath| {
                            clip_path.to_cairo_context(&clip_node, &affine, self)
                        })
                        .map_err(|e| {
                            original_cr.restore();
                            e
                        })?;
                }

                if let Some(mask) = mask {
                    if let Some(acquired) =
                        self.get_acquired_node_of_type(Some(mask), NodeType::Mask)
                    {
                        let node = acquired.get();

                        res = res.and_then(|_| {
                            node.with_impl(|mask: &NodeMask| {
                                mask.generate_cairo_mask(&node, &affine, self)
                            })
                        });
                    } else {
                        rsvg_log!(
                            "element {} references nonexistent mask \"{}\"",
                            node.get_human_readable_name(),
                            mask,
                        );
                    }
                } else if opacity < 1.0 {
                    original_cr.paint_with_alpha(opacity);
                } else {
                    original_cr.paint();
                }

                let bbox = self.bbox;
                self.bbox = self.bbox_stack.pop().unwrap();
                self.bbox.insert(&bbox);
            }

            original_cr.restore();

            res
        }
    }

    fn run_filter(
        &mut self,
        filter_uri: &Fragment,
        node: &RsvgNode,
        values: &ComputedValues,
        child_surface: &cairo::ImageSurface,
    ) -> Result<cairo::ImageSurface, RenderingError> {
        let output = self.surfaces_stack.pop().unwrap();

        match self.get_acquired_node_of_type(Some(filter_uri), NodeType::Filter) {
            Some(acquired) => {
                let filter_node = acquired.get();

                if !filter_node.is_in_error() {
                    // FIXME: deal with out of memory here
                    filters::render(&filter_node, values, &output, self)
                } else {
                    Ok(child_surface.clone())
                }
            }

            None => {
                rsvg_log!(
                    "element {} will not be rendered since its filter \"{}\" was not found",
                    node.get_human_readable_name(),
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
        let mut had_paint_server;

        match *ps {
            PaintServer::Iri {
                ref iri,
                ref alternate,
            } => {
                had_paint_server = false;

                if let Some(acquired) = self.get_acquired_node(iri) {
                    let node = acquired.get();

                    if node.get_type() == NodeType::LinearGradient
                        || node.get_type() == NodeType::RadialGradient
                    {
                        had_paint_server = node.with_impl(|n: &NodeGradient| {
                            n.resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)
                        })?;
                    } else if node.get_type() == NodeType::Pattern {
                        had_paint_server = node.with_impl(|n: &NodePattern| {
                            n.resolve_fallbacks_and_set_pattern(&node, self, opacity, bbox)
                        })?;
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
            }

            PaintServer::SolidColor(color) => {
                self.set_color(&color, opacity, current_color);
                had_paint_server = true;
            }

            PaintServer::None => {
                had_paint_server = false;
            }
        };

        Ok(had_paint_server)
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

    pub fn set_affine_on_cr(&self, cr: &cairo::Context) {
        let (x0, y0) = self.get_offset();
        let affine = cr.get_matrix();
        let matrix = cairo::Matrix::new(
            affine.xx,
            affine.yx,
            affine.xy,
            affine.yy,
            affine.x0 + x0,
            affine.y0 + y0,
        );
        cr.set_matrix(matrix);
    }

    pub fn clip(&self, x: f64, y: f64, w: f64, h: f64) {
        let cr = self.get_cairo_context();
        let save_affine = cr.get_matrix();

        self.set_affine_on_cr(&cr);

        cr.rectangle(x, y, w, h);
        cr.clip();
        cr.set_matrix(save_affine);
    }

    pub fn get_snapshot(&self, surface: &cairo::ImageSurface) {
        let (x, y) = (self.rect.x, self.rect.y);

        // TODO: as far as I can tell this should not render elements past the last (topmost) one
        // with enable-background: new (because technically we shouldn't have been caching them).
        // Right now there are no enable-background checks whatsoever.
        let cr = cairo::Context::new(&surface);
        for draw in self.cr_stack.iter() {
            let nested = self.is_cairo_context_nested(&draw);
            cr.set_source_surface(
                &draw.get_target(),
                if nested { 0f64 } else { -x },
                if nested { 0f64 } else { -y },
            );
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
        let save_initial_cr = self.initial_cr.clone();
        let save_rect = self.rect;
        let save_affine = self.get_cairo_context().get_matrix();

        let cr = cairo::Context::new(surface);
        cr.set_matrix(save_affine);

        self.cr = cr;
        self.initial_cr = self.cr.clone();
        self.rect.x = 0.0;
        self.rect.y = 0.0;
        self.rect.width = width;
        self.rect.height = height;

        let res = self.draw_node_from_stack(cascaded, node, false);

        self.cr = save_cr;
        self.initial_cr = save_initial_cr;
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
            Rc::ptr_eq(top, node)
        } else {
            true
        };

        let values = cascaded.get();
        let res = if draw && values.is_visible() {
            node.draw(node, cascaded, self, clipping)
        } else {
            Ok(())
        };

        if let Some(top) = stack_top {
            self.drawsub_stack.push(top);
        }

        res.and_then(|_| self.check_limits())
    }

    pub fn mask_surface(&mut self, mask: &cairo::ImageSurface) {
        let cr = self.get_cairo_context();

        cr.identity_matrix();

        let (xofs, yofs) = self.get_offset();
        cr.mask_surface(&mask, xofs, yofs);
    }

    pub fn add_node_and_ancestors_to_stack(&mut self, node: &RsvgNode) {
        self.drawsub_stack.push(node.clone());
        if let Some(ref parent) = node.get_parent() {
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

pub struct AcquiredNode(Rc<RefCell<Vec<RsvgNode>>>, RsvgNode);

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        let mut v = self.0.borrow_mut();
        assert!(Rc::ptr_eq(v.last().unwrap(), &self.1));
        v.pop();
    }
}

impl AcquiredNode {
    pub fn get(&self) -> &RsvgNode {
        &self.1
    }
}

/// Keeps a stack of nodes and can check if a certain node is contained in the stack
///
/// Sometimes parts of the code cannot plainly use the implicit stack of acquired
/// nodes as maintained by DrawingCtx::get_acquired_node(), and they must keep their
/// own stack of nodes to test for reference cycles.  NodeStack can be used to do that.
pub struct NodeStack(Vec<RsvgNode>);

impl NodeStack {
    pub fn new() -> NodeStack {
        NodeStack(Vec::new())
    }

    pub fn push(&mut self, node: &RsvgNode) {
        self.0.push(node.clone());
    }

    pub fn contains(&self, node: &RsvgNode) -> bool {
        self.0.iter().find(|n| Rc::ptr_eq(n, node)).is_some()
    }
}
