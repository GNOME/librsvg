use cairo;
use cairo::MatrixTrait;
use cairo_sys;
use glib::translate::*;
use glib_sys;
use libc;
use pango::{self, FontMapExt};
use pango_cairo_sys;
use pangocairo;
use std::cell::RefCell;

use bbox::BoundingBox;
use clip_path::{ClipPathUnits, NodeClipPath};
use coord_units::CoordUnits;
use defs::{self, RsvgDefs};
use filters::filter_render;
use mask::NodeMask;
use node::{rc_node_ptr_eq, CascadedValues, NodeType, RsvgNode};
use rect::RectangleExt;
use state::{CompOp, ComputedValues, EnableBackground};
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
        cr.to_raw_none() == self.initial_cr.to_raw_none()
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
            .find(|n| rc_node_ptr_eq(n, node))
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
            let filter = values.filter.0.get();
            let mask = values.mask.0.get();

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

                        // The bbox rect can be None, for example, if a filter is applied to an
                        // empty group. There is nothing to filter in this case.
                        self.bbox.rect.and_then(|_| {
                            self.get_acquired_node_of_type(filter, NodeType::Filter)
                                .and_then(|acquired| {
                                    let filter_node = acquired.get();

                                    if !filter_node.is_in_error() {
                                        // FIXME: deal with out of memory here
                                        Some(filter_render(
                                            &filter_node,
                                            node,
                                            &output,
                                            self,
                                            "2103".as_ptr() as *const i8,
                                        ))
                                    } else {
                                        None
                                    }
                                })
                        })
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
        let mut draw = false;
        if let Some(top) = self.drawsub_stack.pop() {
            if rc_node_ptr_eq(&top, node) {
                draw = true;
            }

            self.drawsub_stack.push(top);
        } else {
            draw = true;
        }

        if draw {
            let values = cascaded.get();
            if values.is_visible() {
                node.draw(node, cascaded, self, clipping);
            }
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
            assert!(rc_node_ptr_eq(v.last().unwrap(), &self.1));
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
