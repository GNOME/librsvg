use cairo;
use cairo::MatrixTrait;
use cairo_sys;
use glib::translate::*;
use glib_sys;
use libc;
use pango::{self, ContextExt, FontMapExt, LayoutExt};
use pango_cairo_sys;
use pango_sys;
use pangocairo;
use std::cell::RefCell;
use std::rc::Rc;

use bbox::BoundingBox;
use clip_path::{ClipPathUnits, NodeClipPath};
use coord_units::CoordUnits;
use defs::{self, RsvgDefs};
use filters;
use float_eq_cairo::ApproxEqCairo;
use length::Dasharray;
use mask::NodeMask;
use node::{CascadedValues, NodeType, RsvgNode};
use paint_server::{self, PaintServer};
use rect::RectangleExt;
use state::{
    ClipRule,
    CompOp,
    ComputedValues,
    EnableBackground,
    FillRule,
    ShapeRendering,
    StrokeDasharray,
    StrokeLinecap,
    StrokeLinejoin,
    TextRendering,
};
use unitinterval::UnitInterval;
use viewbox::ViewBox;

pub enum RsvgDrawingCtx {}

pub struct DrawingCtx {
    rect: cairo::Rectangle,
    dpi_x: f64,
    dpi_y: f64,

    cr_stack: Vec<cairo::Context>,
    cr: cairo::Context,
    initial_cr: cairo::Context,

    surfaces_stack: Vec<cairo::ImageSurface>,

    vb: ViewBox,
    vb_stack: Vec<ViewBox>,

    bbox: BoundingBox,
    bbox_stack: Vec<BoundingBox>,

    drawsub_stack: Vec<RsvgNode>,

    defs: *const RsvgDefs,
    acquired_nodes: RefCell<Vec<RsvgNode>>,

    is_testing: bool,
}

impl<'a> DrawingCtx {
    pub fn new(
        cr: cairo::Context,
        width: f64,
        height: f64,
        vb_width: f64,
        vb_height: f64,
        dpi_x: f64,
        dpi_y: f64,
        defs: *const RsvgDefs,
        is_testing: bool,
    ) -> DrawingCtx {
        let mut affine = cr.get_matrix();
        let rect = cairo::Rectangle {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }.transform(&affine)
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

        DrawingCtx {
            rect,
            dpi_x,
            dpi_y,
            cr_stack: Vec::new(),
            cr: cr.clone(),
            initial_cr: cr.clone(),
            surfaces_stack: Vec::new(),
            vb: ViewBox::new(0.0, 0.0, vb_width, vb_height),
            vb_stack: Vec::new(),
            bbox: BoundingBox::new(&affine),
            bbox_stack: Vec::new(),
            drawsub_stack: Vec::new(),
            defs,
            acquired_nodes: RefCell::new(Vec::new()),
            is_testing,
        }
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

    pub fn is_cairo_context_nested(&self, cr: &cairo::Context) -> bool {
        cr.to_raw_none() != self.initial_cr.to_raw_none()
    }

    pub fn get_cr_stack(&self) -> &Vec<cairo::Context> {
        &self.cr_stack
    }

    pub fn get_width(&self) -> f64 {
        self.rect.width
    }

    pub fn get_height(&self) -> f64 {
        self.rect.height
    }

    pub fn get_raw_offset(&self) -> (f64, f64) {
        (self.rect.x, self.rect.y)
    }

    pub fn get_offset(&self) -> (f64, f64) {
        if self.is_cairo_context_nested(&self.get_cairo_context()) {
            (0.0, 0.0)
        } else {
            (self.rect.x, self.rect.y)
        }
    }

    pub fn get_dpi(&self) -> (f64, f64) {
        (self.dpi_x, self.dpi_y)
    }

    pub fn get_view_box_size(&self) -> (f64, f64) {
        (self.vb.0.width, self.vb.0.height)
    }

    pub fn push_view_box(&mut self, width: f64, height: f64) {
        self.vb_stack.push(self.vb);
        self.vb = ViewBox::new(0.0, 0.0, width, height);
    }

    pub fn pop_view_box(&mut self) {
        self.vb = self.vb_stack.pop().unwrap();
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
    pub fn get_acquired_node(&mut self, url: &str) -> Option<AcquiredNode> {
        if let Some(node) = defs::lookup(self.defs, url) {
            if !self.acquired_nodes_contains(node) {
                self.acquired_nodes.borrow_mut().push(node.clone());
                return Some(AcquiredNode(&self.acquired_nodes as *const _, node.clone()));
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
    pub fn get_acquired_node_of_type(
        &mut self,
        url: Option<&str>,
        node_type: NodeType,
    ) -> Option<AcquiredNode> {
        url.and_then(move |url| self.get_acquired_node(url))
            .and_then(|acquired| {
                if acquired.get().get_type() == node_type {
                    Some(acquired)
                } else {
                    None
                }
            })
    }

    pub fn with_discrete_layer(
        &mut self,
        node: &RsvgNode,
        values: &ComputedValues,
        clipping: bool,
        draw_fn: &mut FnMut(&mut DrawingCtx),
    ) {
        if clipping {
            draw_fn(self);
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
            let comp_op = values.comp_op;
            let enable_background = values.enable_background;

            let affine = original_cr.get_matrix();

            let (clip_node, clip_units) = {
                let clip_node = self
                    .get_acquired_node_of_type(clip_uri, NodeType::ClipPath)
                    .and_then(|acquired| Some(acquired.get()));

                let mut clip_units = Default::default();

                if let Some(ref clip_node) = clip_node {
                    clip_node.with_impl(|clip_path: &NodeClipPath| {
                        let ClipPathUnits(u) = clip_path.get_units();
                        clip_units = Some(u);
                    });
                }

                (clip_node, clip_units)
            };

            if clip_units == Some(CoordUnits::UserSpaceOnUse) {
                if let Some(ref clip_node) = clip_node {
                    clip_node.with_impl(|clip_path: &NodeClipPath| {
                        clip_path.to_cairo_context(clip_node, &affine, self);
                    });
                }
            }

            let needs_temporary_surface = !(opacity == 1.0
                && filter.is_none()
                && mask.is_none()
                && (clip_units == None || clip_units == Some(CoordUnits::UserSpaceOnUse))
                && comp_op == CompOp::SrcOver
                && enable_background == EnableBackground::Accumulate);

            let child_surface = {
                if needs_temporary_surface {
                    // FIXME: in the following, we unwrap() the result of
                    // ImageSurface::create().  We have to decide how to handle
                    // out-of-memory here.
                    let surface = cairo::ImageSurface::create(
                        cairo::Format::ARgb32,
                        self.rect.width as i32,
                        self.rect.height as i32,
                    ).unwrap();

                    if filter.is_some() {
                        self.surfaces_stack.push(surface.clone());
                    }

                    let cr = cairo::Context::new(&surface);
                    cr.set_matrix(affine);

                    self.cr_stack.push(self.cr.clone());
                    self.cr = cr.clone();

                    self.bbox_stack.push(self.bbox);
                    self.bbox = BoundingBox::new(&affine);

                    surface
                } else {
                    cairo::ImageSurface::from(original_cr.get_target()).unwrap()
                }
            };

            draw_fn(self);

            if needs_temporary_surface {
                let filter_result_surface = filter
                    .and_then(|_| {
                        // About the following unwrap(), see the FIXME above.  We should be pushing
                        // only surfaces that are not in an error state, but currently we don't
                        // actually ensure that.
                        let output = self.surfaces_stack.pop().unwrap();

                        match self.get_acquired_node_of_type(filter, NodeType::Filter) {
                            Some(acquired) => {
                                let filter_node = acquired.get();

                                if !filter_node.is_in_error() {
                                    // FIXME: deal with out of memory here
                                    Some(filters::render(&filter_node, node, &output, self))
                                } else {
                                    None
                                }
                            }
                            None => {
                                // Non-existing filters must act as null filters (that is, an
                                // empty surface is returned).
                                // TODO: handle the error properly and not via expect().
                                Some(
                                    cairo::ImageSurface::create(
                                        cairo::Format::ARgb32,
                                        child_surface.get_width(),
                                        child_surface.get_height(),
                                    ).expect("couldn't create an empty surface"),
                                )
                            }
                        }
                    })
                    .or(Some(child_surface))
                    .unwrap();

                self.cr = self.cr_stack.pop().unwrap();

                let (xofs, yofs) = self.get_offset();

                original_cr.identity_matrix();
                original_cr.set_source_surface(&filter_result_surface, xofs, yofs);

                if clip_units == Some(CoordUnits::ObjectBoundingBox) {
                    if let Some(ref clip_node) = clip_node {
                        clip_node.with_impl(|clip_path: &NodeClipPath| {
                            clip_path.to_cairo_context(clip_node, &affine, self);
                        });
                    }
                }

                original_cr.set_operator(cairo::Operator::from(comp_op));

                if let Some(mask) = mask {
                    if let Some(acquired) =
                        self.get_acquired_node_of_type(Some(mask), NodeType::Mask)
                    {
                        let node = acquired.get();

                        node.with_impl(|mask: &NodeMask| {
                            mask.generate_cairo_mask(&node, &affine, self);
                        });
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
        }
    }

    pub fn get_pango_context(&self) -> pango::Context {
        let font_map = pangocairo::FontMap::get_default().unwrap();
        let context = font_map.create_context().unwrap();
        let cr = self.get_cairo_context();
        pangocairo::functions::update_context(&cr, &context);

        set_resolution(&context, self.dpi_x);

        if self.is_testing {
            let mut options = cairo::FontOptions::new();

            options.set_antialias(cairo::Antialias::Gray);
            options.set_hint_style(cairo::enums::HintStyle::Full);
            options.set_hint_metrics(cairo::enums::HintMetrics::On);

            set_font_options(&context, &options);
        }

        context
    }

    pub fn draw_pango_layout(
        &mut self,
        layout: &pango::Layout,
        values: &ComputedValues,
        x: f64,
        y: f64,
        clipping: bool,
    ) {
        let (ink, _) = layout.get_extents();

        if ink.width == 0 || ink.height == 0 {
            return;
        }

        let cr = self.get_cairo_context();
        cr.save();

        self.set_affine_on_cr(&cr);

        let affine = cr.get_matrix();

        let gravity = layout.get_context().unwrap().get_gravity();
        let bbox = compute_text_bbox(&ink, x, y, &affine, gravity);

        if !clipping {
            self.insert_bbox(&bbox);
        }

        cr.set_antialias(cairo::Antialias::from(values.text_rendering));

        self.setup_cr_for_stroke(&cr, values);

        let rotation = unsafe { pango_sys::pango_gravity_to_rotation(gravity.to_glib()) };

        cr.move_to(x, y);
        if !rotation.approx_eq_cairo(&0.0) {
            cr.rotate(-rotation);
        }

        let current_color = &values.color.0;

        let fill_opacity = &values.fill_opacity.0;

        if !clipping {
            if paint_server::set_source_paint_server(
                self,
                &values.fill.0,
                fill_opacity,
                &bbox,
                current_color,
            ) {
                pangocairo::functions::update_layout(&cr, layout);
                pangocairo::functions::show_layout(&cr, layout);
            }
        }

        let stroke_opacity = &values.stroke_opacity.0;

        let mut need_layout_path = clipping;

        if !clipping {
            if paint_server::set_source_paint_server(
                self,
                &values.stroke.0,
                stroke_opacity,
                &bbox,
                &current_color,
            ) {
                need_layout_path = true;
            }
        }

        if need_layout_path {
            pangocairo::functions::update_layout(&cr, layout);
            pangocairo::functions::layout_path(&cr, layout);

            if !clipping {
                let ib = BoundingBox::new(&affine).with_ink_extents(cr.stroke_extents());
                cr.stroke();
                self.insert_bbox(&ib);
            }
        }

        cr.restore();
    }

    fn setup_cr_for_stroke(&self, cr: &cairo::Context, values: &ComputedValues) {
        cr.set_line_width(values.stroke_width.0.normalize(values, self));
        cr.set_miter_limit(values.stroke_miterlimit.0);
        cr.set_line_cap(cairo::LineCap::from(values.stroke_line_cap));
        cr.set_line_join(cairo::LineJoin::from(values.stroke_line_join));

        if let StrokeDasharray(Dasharray::Array(ref dashes)) = values.stroke_dasharray {
            let normalized_dashes: Vec<f64> =
                dashes.iter().map(|l| l.normalize(values, self)).collect();

            let total_length = normalized_dashes.iter().fold(0.0, |acc, &len| acc + len);

            if total_length > 0.0 {
                let offset = values.stroke_dashoffset.0.normalize(values, self);
                cr.set_dash(&normalized_dashes, offset);
            } else {
                cr.set_dash(&[], 0.0);
            }
        }
    }

    pub fn stroke_and_fill(&mut self, cr: &cairo::Context, values: &ComputedValues) {
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

        if paint_server::set_source_paint_server(
            self,
            &values.fill.0,
            fill_opacity,
            &bbox,
            current_color,
        ) {
            if values.stroke.0 == PaintServer::None {
                cr.fill();
            } else {
                cr.fill_preserve();
            }
        }

        let stroke_opacity = values.stroke_opacity.0;

        if paint_server::set_source_paint_server(
            self,
            &values.stroke.0,
            &stroke_opacity,
            &bbox,
            &current_color,
        ) {
            cr.stroke();
        }

        // clear the path in case stroke == fill == None; otherwise
        // we leave it around from computing the bounding box
        cr.new_path();
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

    pub fn clip(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let cr = self.get_cairo_context();
        let save_affine = cr.get_matrix();

        self.set_affine_on_cr(&cr);

        cr.rectangle(x, y, w, h);
        cr.clip();
        cr.set_matrix(save_affine);
    }

    pub fn draw_node_on_surface(
        &mut self,
        node: &RsvgNode,
        cascaded: &CascadedValues,
        surface: &cairo::ImageSurface,
        width: f64,
        height: f64,
    ) {
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

        self.draw_node_from_stack(cascaded, node, false);

        self.cr = save_cr;
        self.initial_cr = save_initial_cr;
        self.rect = save_rect;
    }

    pub fn draw_node_from_stack(
        &mut self,
        cascaded: &CascadedValues,
        node: &RsvgNode,
        clipping: bool,
    ) {
        let mut draw = true;

        let stack_top = self.drawsub_stack.pop();

        if let Some(ref top) = stack_top {
            if !Rc::ptr_eq(top, node) {
                draw = false;
            }
        }

        if draw {
            let values = cascaded.get();
            if values.is_visible() {
                node.draw(node, cascaded, self, clipping);
            }
        }

        if let Some(top) = stack_top {
            self.drawsub_stack.push(top);
        }
    }

    pub fn add_node_and_ancestors_to_stack(&mut self, node: &RsvgNode) {
        self.drawsub_stack.push(node.clone());
        if let Some(ref parent) = node.get_parent() {
            self.add_node_and_ancestors_to_stack(parent);
        }
    }
}

// remove this binding once pangocairo-rs has ContextExt::set_resolution()
fn set_resolution(context: &pango::Context, dpi: f64) {
    unsafe {
        pango_cairo_sys::pango_cairo_context_set_resolution(context.to_glib_none().0, dpi);
    }
}

// remove this binding once pangocairo-rs has ContextExt::set_font_options()
fn set_font_options(context: &pango::Context, options: &cairo::FontOptions) {
    unsafe {
        pango_cairo_sys::pango_cairo_context_set_font_options(
            context.to_glib_none().0,
            options.to_glib_none().0,
        );
    }
}

// FIXME: should the pango crate provide this like PANGO_GRAVITY_IS_VERTICAL() ?
fn gravity_is_vertical(gravity: pango::Gravity) -> bool {
    match gravity {
        pango::Gravity::East | pango::Gravity::West => true,
        _ => false,
    }
}

fn compute_text_bbox(
    ink: &pango::Rectangle,
    x: f64,
    y: f64,
    affine: &cairo::Matrix,
    gravity: pango::Gravity,
) -> BoundingBox {
    let pango_scale = f64::from(pango::SCALE);

    let mut bbox = BoundingBox::new(affine);

    let ink_x = f64::from(ink.x);
    let ink_y = f64::from(ink.y);
    let ink_width = f64::from(ink.width);
    let ink_height = f64::from(ink.height);

    if gravity_is_vertical(gravity) {
        bbox.rect = Some(cairo::Rectangle {
            x: x + (ink_x - ink_height) / pango_scale,
            y: y + ink_y / pango_scale,
            width: ink_height / pango_scale,
            height: ink_width / pango_scale,
        });
    } else {
        bbox.rect = Some(cairo::Rectangle {
            x: x + ink_x / pango_scale,
            y: y + ink_y / pango_scale,
            width: ink_width / pango_scale,
            height: ink_height / pango_scale,
        });
    }

    bbox
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

impl From<CompOp> for cairo::Operator {
    fn from(op: CompOp) -> cairo::Operator {
        match op {
            CompOp::Clear => cairo::Operator::Clear,
            CompOp::Src => cairo::Operator::Source,
            CompOp::Dst => cairo::Operator::Dest,
            CompOp::SrcOver => cairo::Operator::Over,
            CompOp::DstOver => cairo::Operator::DestOver,
            CompOp::SrcIn => cairo::Operator::In,
            CompOp::DstIn => cairo::Operator::DestIn,
            CompOp::SrcOut => cairo::Operator::Out,
            CompOp::DstOut => cairo::Operator::DestOut,
            CompOp::SrcAtop => cairo::Operator::Atop,
            CompOp::DstAtop => cairo::Operator::DestAtop,
            CompOp::Xor => cairo::Operator::Xor,
            CompOp::Plus => cairo::Operator::Add,
            CompOp::Multiply => cairo::Operator::Multiply,
            CompOp::Screen => cairo::Operator::Screen,
            CompOp::Overlay => cairo::Operator::Overlay,
            CompOp::Darken => cairo::Operator::Darken,
            CompOp::Lighten => cairo::Operator::Lighten,
            CompOp::ColorDodge => cairo::Operator::ColorDodge,
            CompOp::ColorBurn => cairo::Operator::ColorBurn,
            CompOp::HardLight => cairo::Operator::HardLight,
            CompOp::SoftLight => cairo::Operator::SoftLight,
            CompOp::Difference => cairo::Operator::Difference,
            CompOp::Exclusion => cairo::Operator::Exclusion,
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

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_draw_node_from_stack(
    raw_draw_ctx: *mut RsvgDrawingCtx,
    raw_node: *const RsvgNode,
    raw_cascade_from: *const RsvgNode,
    clipping: glib_sys::gboolean,
) {
    assert!(!raw_draw_ctx.is_null());
    let draw_ctx = unsafe { &mut *(raw_draw_ctx as *mut DrawingCtx) };

    assert!(!raw_node.is_null());
    let node = unsafe { &*raw_node };

    let cascade_from = if raw_cascade_from.is_null() {
        None
    } else {
        Some(unsafe { &*raw_cascade_from })
    };

    let clipping: bool = from_glib(clipping);

    let cascaded = match cascade_from {
        None => node.get_cascaded_values(),
        Some(n) => {
            let c = n.get_cascaded_values();
            let v = c.get();
            CascadedValues::new_from_values(node, v)
        }
    };

    draw_ctx.draw_node_from_stack(&cascaded, node, clipping);
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_add_node_and_ancestors_to_stack(
    raw_draw_ctx: *const RsvgDrawingCtx,
    raw_node: *const RsvgNode,
) {
    assert!(!raw_draw_ctx.is_null());
    let draw_ctx = unsafe { &mut *(raw_draw_ctx as *mut DrawingCtx) };

    assert!(!raw_node.is_null());
    let node = unsafe { &*raw_node };

    draw_ctx.add_node_and_ancestors_to_stack(node);
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_get_ink_rect(
    raw_draw_ctx: *const RsvgDrawingCtx,
    ink_rect: *mut cairo_sys::cairo_rectangle_t,
) {
    assert!(!raw_draw_ctx.is_null());
    let draw_ctx = unsafe { &mut *(raw_draw_ctx as *mut DrawingCtx) };

    assert!(!ink_rect.is_null());

    let r = draw_ctx.get_bbox().ink_rect.unwrap();
    unsafe {
        (*ink_rect).x = r.x;
        (*ink_rect).y = r.y;
        (*ink_rect).width = r.width;
        (*ink_rect).height = r.height;
    }
}

pub struct AcquiredNode(*const RefCell<Vec<RsvgNode>>, RsvgNode);

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        unsafe {
            let mut v = (*self.0).borrow_mut();
            assert!(Rc::ptr_eq(v.last().unwrap(), &self.1));
            v.pop();
        }
    }
}

impl AcquiredNode {
    pub fn get(&self) -> RsvgNode {
        self.1.clone()
    }
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_new(
    cr: *mut cairo_sys::cairo_t,
    width: u32,
    height: u32,
    vb_width: libc::c_double,
    vb_height: libc::c_double,
    dpi_x: libc::c_double,
    dpi_y: libc::c_double,
    defs: *const RsvgDefs,
    is_testing: glib_sys::gboolean,
) -> *mut RsvgDrawingCtx {
    Box::into_raw(Box::new(DrawingCtx::new(
        unsafe { from_glib_none(cr) },
        f64::from(width),
        f64::from(height),
        vb_width,
        vb_height,
        dpi_x,
        dpi_y,
        defs,
        from_glib(is_testing),
    ))) as *mut RsvgDrawingCtx
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_free(raw_draw_ctx: *mut RsvgDrawingCtx) {
    assert!(!raw_draw_ctx.is_null());
    let draw_ctx = unsafe { &mut *(raw_draw_ctx as *mut DrawingCtx) };

    unsafe {
        Box::from_raw(draw_ctx);
    }
}
