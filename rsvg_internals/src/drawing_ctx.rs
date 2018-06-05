use std::ptr;

use cairo;
use cairo_sys;
use glib::translate::*;
use glib_sys;
use libc;
use pango::{self, FontMapExt};
use pango_cairo_sys;
use pangocairo;

use bbox::{BoundingBox, RsvgBbox};
use clip_path::{ClipPathUnits, NodeClipPath};
use coord_units::CoordUnits;
use filters::filter_render;
use iri::IRI;
use mask::NodeMask;
use node::{CascadedValues, NodeType, RsvgNode};
use rect::RectangleExt;
use state::{ClipPath, CompOp, ComputedValues, EnableBackground, Filter, Mask};
use unitinterval::UnitInterval;

pub enum RsvgDrawingCtx {}

#[allow(improper_ctypes)]
extern "C" {
    fn rsvg_drawing_ctx_get_cairo_context(
        draw_ctx: *const RsvgDrawingCtx,
    ) -> *mut cairo_sys::cairo_t;

    fn rsvg_drawing_ctx_set_cairo_context(
        draw_ctx: *const RsvgDrawingCtx,
        cr: *const cairo_sys::cairo_t,
    );

    fn rsvg_drawing_ctx_get_dpi(
        draw_ctx: *const RsvgDrawingCtx,
        out_dpi_x: *mut f64,
        out_dpi_y: *mut f64,
    );

    fn rsvg_drawing_ctx_get_view_box_size(
        draw_ctx: *const RsvgDrawingCtx,
        out_x: *mut f64,
        out_y: *mut f64,
    );

    fn rsvg_drawing_ctx_push_view_box(draw_ctx: *const RsvgDrawingCtx, width: f64, height: f64);

    fn rsvg_drawing_ctx_pop_view_box(draw_ctx: *const RsvgDrawingCtx);

    fn rsvg_drawing_ctx_acquire_node(
        draw_ctx: *const RsvgDrawingCtx,
        url: *const libc::c_char,
    ) -> *mut RsvgNode;

    fn rsvg_drawing_ctx_acquire_node_of_type(
        draw_ctx: *const RsvgDrawingCtx,
        url: *const libc::c_char,
        node_type: NodeType,
    ) -> *mut RsvgNode;

    fn rsvg_drawing_ctx_release_node(draw_ctx: *const RsvgDrawingCtx, node: *mut RsvgNode);

    fn rsvg_drawing_ctx_get_offset(
        draw_ctx: *const RsvgDrawingCtx,
        out_x: *mut f64,
        out_y: *mut f64,
    );

    fn rsvg_drawing_ctx_get_bbox(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgBbox;

    fn rsvg_drawing_ctx_is_testing(draw_ctx: *const RsvgDrawingCtx) -> glib_sys::gboolean;
}

pub fn get_cairo_context(draw_ctx: *const RsvgDrawingCtx) -> cairo::Context {
    unsafe {
        let raw_cr = rsvg_drawing_ctx_get_cairo_context(draw_ctx);

        cairo::Context::from_glib_none(raw_cr)
    }
}

pub fn set_cairo_context(draw_ctx: *const RsvgDrawingCtx, cr: &cairo::Context) {
    unsafe {
        let raw_cr = cr.to_glib_none().0;

        rsvg_drawing_ctx_set_cairo_context(draw_ctx, raw_cr);
    }
}

pub fn get_dpi(draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut dpi_x: f64 = 0.0;
    let mut dpi_y: f64 = 0.0;

    unsafe {
        rsvg_drawing_ctx_get_dpi(draw_ctx, &mut dpi_x, &mut dpi_y);
    }

    (dpi_x, dpi_y)
}

pub fn get_view_box_size(draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut w: f64 = 0.0;
    let mut h: f64 = 0.0;

    unsafe {
        rsvg_drawing_ctx_get_view_box_size(draw_ctx, &mut w, &mut h);
    }

    (w, h)
}

pub fn push_view_box(draw_ctx: *const RsvgDrawingCtx, width: f64, height: f64) {
    unsafe {
        rsvg_drawing_ctx_push_view_box(draw_ctx, width, height);
    }
}

pub fn pop_view_box(draw_ctx: *const RsvgDrawingCtx) {
    unsafe {
        rsvg_drawing_ctx_pop_view_box(draw_ctx);
    }
}

pub fn get_acquired_node(draw_ctx: *const RsvgDrawingCtx, url: &str) -> Option<AcquiredNode> {
    let raw_node = unsafe { rsvg_drawing_ctx_acquire_node(draw_ctx, str::to_glib_none(url).0) };

    if raw_node.is_null() {
        None
    } else {
        Some(AcquiredNode(draw_ctx, raw_node))
    }
}

pub fn get_acquired_node_of_type(
    draw_ctx: *const RsvgDrawingCtx,
    url: &str,
    node_type: NodeType,
) -> Option<AcquiredNode> {
    let raw_node = unsafe {
        rsvg_drawing_ctx_acquire_node_of_type(draw_ctx, str::to_glib_none(url).0, node_type)
    };

    if raw_node.is_null() {
        None
    } else {
        Some(AcquiredNode(draw_ctx, raw_node))
    }
}

pub fn push_discrete_layer(draw_ctx: *mut RsvgDrawingCtx, values: &ComputedValues, clipping: bool) {
    if !clipping {
        get_cairo_context(draw_ctx).save();
        push_render_stack(draw_ctx, values);
    }
}

pub fn pop_discrete_layer(
    draw_ctx: *mut RsvgDrawingCtx,
    node: &RsvgNode,
    values: &ComputedValues,
    clipping: bool,
) {
    if !clipping {
        pop_render_stack(draw_ctx, node, values);
        get_cairo_context(draw_ctx).restore();
    }
}

pub fn get_width(draw_ctx: *const RsvgDrawingCtx) -> f64 {
    unsafe { rsvg_drawing_ctx_get_width(draw_ctx) }
}

pub fn get_height(draw_ctx: *const RsvgDrawingCtx) -> f64 {
    unsafe { rsvg_drawing_ctx_get_height(draw_ctx) }
}

pub fn get_offset(draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut w: f64 = 0.0;
    let mut h: f64 = 0.0;

    unsafe {
        rsvg_drawing_ctx_get_offset(draw_ctx, &mut w, &mut h);
    }

    (w, h)
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

pub fn get_pango_context(draw_ctx: *const RsvgDrawingCtx) -> pango::Context {
    let font_map = pangocairo::FontMap::get_default().unwrap();
    let context = font_map.create_context().unwrap();
    let cr = get_cairo_context(draw_ctx);
    pangocairo::functions::update_context(&cr, &context);

    set_resolution(&context, get_dpi(draw_ctx).1);

    let testing = unsafe { from_glib(rsvg_drawing_ctx_is_testing(draw_ctx)) };
    if testing {
        let mut options = cairo::FontOptions::new();

        options.set_antialias(cairo::Antialias::Gray);
        options.set_hint_style(cairo::enums::HintStyle::Full);
        options.set_hint_metrics(cairo::enums::HintMetrics::On);

        set_font_options(&context, &options);
    }

    context
}

pub fn insert_bbox(draw_ctx: *const RsvgDrawingCtx, bbox: &BoundingBox) {
    let draw_ctx_bbox = get_bbox_mut(draw_ctx);

    draw_ctx_bbox.insert(bbox);
}

pub fn set_bbox(draw_ctx: *mut RsvgDrawingCtx, bbox: &BoundingBox) {
    let draw_ctx_bbox = get_bbox_mut(draw_ctx);

    *draw_ctx_bbox = *bbox;
}

pub fn get_bbox_mut<'a>(draw_ctx: *const RsvgDrawingCtx) -> &'a mut BoundingBox {
    unsafe {
        let bb = rsvg_drawing_ctx_get_bbox(draw_ctx);
        &mut *(bb as *mut BoundingBox)
    }
}

pub fn get_bbox<'a>(draw_ctx: *const RsvgDrawingCtx) -> &'a BoundingBox {
    get_bbox_mut(draw_ctx)
}

extern "C" {
    fn rsvg_drawing_ctx_get_width(draw_ctx: *const RsvgDrawingCtx) -> f64;
    fn rsvg_drawing_ctx_get_height(draw_ctx: *const RsvgDrawingCtx) -> f64;

    fn rsvg_drawing_ctx_push_surface(
        draw_ctx: *mut RsvgDrawingCtx,
        surface: *const cairo_sys::cairo_surface_t,
    );
    fn rsvg_drawing_ctx_pop_surface(
        draw_ctx: *mut RsvgDrawingCtx,
    ) -> *mut cairo_sys::cairo_surface_t;

    fn rsvg_drawing_ctx_push_cr(draw_ctx: *mut RsvgDrawingCtx, cr: *mut cairo_sys::cairo_t);
    fn rsvg_drawing_ctx_pop_cr(draw_ctx: *mut RsvgDrawingCtx);

    fn rsvg_drawing_ctx_push_bounding_box(draw_ctx: *mut RsvgDrawingCtx);
    fn rsvg_drawing_ctx_pop_bounding_box(draw_ctx: *mut RsvgDrawingCtx);
}

fn push_render_stack(draw_ctx: *mut RsvgDrawingCtx, values: &ComputedValues) {
    let clip_path = match values.clip_path {
        ClipPath(IRI::Resource(ref p)) => Some(p),
        _ => None,
    };

    let filter = match values.filter {
        Filter(IRI::Resource(ref f)) => Some(f),
        _ => None,
    };

    let mask = match values.mask {
        Mask(IRI::Resource(ref m)) => Some(m),
        _ => None,
    };

    let UnitInterval(opacity) = values.opacity.0;
    let comp_op = values.comp_op;
    let enable_background = values.enable_background;

    let mut late_clip = false;

    let current_affine = get_cairo_context(draw_ctx).get_matrix();

    if let Some(clip_path) = clip_path {
        if let Some(acquired) = get_acquired_node_of_type(draw_ctx, clip_path, NodeType::ClipPath) {
            let node = acquired.get();

            node.with_impl(|clip_path: &NodeClipPath| match clip_path.get_units() {
                ClipPathUnits(CoordUnits::UserSpaceOnUse) => {
                    clip_path.to_cairo_context(&node, &current_affine, draw_ctx);
                }

                ClipPathUnits(CoordUnits::ObjectBoundingBox) => {
                    late_clip = true;
                }
            });
        }
    }

    if opacity == 1.0
        && filter.is_none()
        && mask.is_none()
        && !late_clip
        && comp_op == CompOp::SrcOver
        && enable_background == EnableBackground::Accumulate
    {
        return;
    }

    // FIXME: in the following, we unwrap() the result of
    // ImageSurface::create().  We have to decide how to handle
    // out-of-memory here.
    let surface = unsafe {
        cairo::ImageSurface::create(
            cairo::Format::ARgb32,
            rsvg_drawing_ctx_get_width(draw_ctx) as i32,
            rsvg_drawing_ctx_get_height(draw_ctx) as i32,
        ).unwrap()
    };

    if filter.is_some() {
        unsafe {
            rsvg_drawing_ctx_push_surface(draw_ctx, surface.to_glib_none().0);
        }
    }

    let child_cr = cairo::Context::new(&surface);
    child_cr.set_matrix(get_cairo_context(draw_ctx).get_matrix());

    unsafe {
        rsvg_drawing_ctx_push_cr(draw_ctx, child_cr.to_raw_none());
    }

    unsafe {
        rsvg_drawing_ctx_push_bounding_box(draw_ctx);
    }
}

fn pop_render_stack(draw_ctx: *mut RsvgDrawingCtx, node: &RsvgNode, values: &ComputedValues) {
    let child_cr = get_cairo_context(draw_ctx);

    let clip_path = match values.clip_path {
        ClipPath(IRI::Resource(ref p)) => Some(p),
        _ => None,
    };

    let filter = match values.filter {
        Filter(IRI::Resource(ref f)) => Some(f),
        _ => None,
    };

    let mask = match values.mask {
        Mask(IRI::Resource(ref m)) => Some(m),
        _ => None,
    };

    let UnitInterval(opacity) = values.opacity.0;
    let comp_op = values.comp_op;
    let enable_background = values.enable_background;

    let mut late_clip = false;

    if let Some(clip_path) = clip_path {
        if let Some(acquired) = get_acquired_node_of_type(draw_ctx, clip_path, NodeType::ClipPath) {
            let mut clip_path_units = ClipPathUnits::default();

            acquired.get().with_impl(|clip_path: &NodeClipPath| {
                clip_path_units = clip_path.get_units();
            });

            match clip_path_units {
                ClipPathUnits(CoordUnits::UserSpaceOnUse) => {
                    late_clip = false;
                }

                ClipPathUnits(CoordUnits::ObjectBoundingBox) => {
                    late_clip = true;
                }
            }
        }
    }

    if opacity == 1.0
        && filter.is_none()
        && mask.is_none()
        && !late_clip
        && comp_op == CompOp::SrcOver
        && enable_background == EnableBackground::Accumulate
    {
        return;
    }

    let surface = if let Some(filter) = filter {
        // About the following unwrap(), see the FIXME in push_render_stack().  We should
        // be pushing only surfaces that are not in an error state, but currently we don't
        // actually ensure that.
        let output = unsafe {
            cairo::ImageSurface::from_raw_full(rsvg_drawing_ctx_pop_surface(draw_ctx)).unwrap()
        };

        if let Some(acquired) = get_acquired_node_of_type(draw_ctx, filter, NodeType::Filter) {
            filter_render(
                &acquired.get(),
                node,
                &output,
                draw_ctx,
                "2103".as_ptr() as *const i8,
            )
        // FIXME: deal with out of memory here
        } else {
            cairo::ImageSurface::from(child_cr.get_target()).unwrap()
        }
    } else {
        cairo::ImageSurface::from(child_cr.get_target()).unwrap()
    };

    unsafe {
        rsvg_drawing_ctx_pop_cr(draw_ctx);
    }

    let cr = get_cairo_context(draw_ctx);

    let current_affine = cr.get_matrix();

    let (xofs, yofs) = get_offset(draw_ctx);

    cr.identity_matrix();
    cr.set_source_surface(&surface, xofs, yofs);

    if late_clip {
        if let Some(clip_path) = clip_path {
            if let Some(acquired) =
                get_acquired_node_of_type(draw_ctx, clip_path, NodeType::ClipPath)
            {
                let node = acquired.get();

                node.with_impl(|clip_path: &NodeClipPath| {
                    clip_path.to_cairo_context(&node, &current_affine, draw_ctx);
                });
            }
        }
    }

    cr.set_operator(cairo::Operator::from(comp_op));

    if let Some(mask) = mask {
        if let Some(acquired) = get_acquired_node_of_type(draw_ctx, mask, NodeType::Mask) {
            let node = acquired.get();

            node.with_impl(|mask: &NodeMask| {
                mask.generate_cairo_mask(&node, &current_affine, draw_ctx);
            });
        }
    } else if opacity < 1.0 {
        cr.paint_with_alpha(opacity);
    } else {
        cr.paint();
    }

    cr.set_matrix(current_affine);

    unsafe {
        rsvg_drawing_ctx_pop_bounding_box(draw_ctx);
    }
}

extern "C" {
    fn rsvg_drawing_ctx_should_draw_node_from_stack(
        draw_ctx: *const RsvgDrawingCtx,
        raw_node: *const RsvgNode,
        out_stacksave: *mut *const libc::c_void,
    ) -> glib_sys::gboolean;

    fn rsvg_drawing_ctx_restore_stack(
        draw_ctx: *const RsvgDrawingCtx,
        stacksave: *const libc::c_void,
    );

}

pub fn draw_node_from_stack(
    draw_ctx: *mut RsvgDrawingCtx,
    cascaded: &CascadedValues,
    node: &RsvgNode,
    with_layer: bool,
    clipping: bool,
) {
    let mut stacksave = ptr::null();

    unsafe {
        let should_draw = from_glib(rsvg_drawing_ctx_should_draw_node_from_stack(
            draw_ctx,
            node as *const RsvgNode,
            &mut stacksave,
        ));

        if should_draw {
            let values = cascaded.get();
            if values.is_visible() {
                node.draw(node, cascaded, draw_ctx, with_layer, clipping);
            }
        }

        rsvg_drawing_ctx_restore_stack(draw_ctx, stacksave);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_draw_node_from_stack(
    draw_ctx: *mut RsvgDrawingCtx,
    raw_node: *const RsvgNode,
    raw_cascade_from: *const RsvgNode,
    clipping: glib_sys::gboolean,
) {
    assert!(!draw_ctx.is_null());

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

    draw_node_from_stack(draw_ctx, &cascaded, node, true, clipping);
}

pub struct AcquiredNode(*const RsvgDrawingCtx, *mut RsvgNode);

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        unsafe {
            rsvg_drawing_ctx_release_node(self.0, self.1);
        }
    }
}

impl AcquiredNode {
    pub fn get(&self) -> RsvgNode {
        unsafe { (*self.1).clone() }
    }
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_transformed_image_bounding_box(
    affine: *const cairo::Matrix,
    w: f64,
    h: f64,
    bbx: *mut libc::c_double,
    bby: *mut libc::c_double,
    bbw: *mut libc::c_double,
    bbh: *mut libc::c_double,
) {
    let affine = unsafe { &*affine };
    let r = cairo::Rectangle {
        x: 0.0,
        y: 0.0,
        width: w,
        height: h,
    }.transform(affine)
        .outer();

    unsafe {
        *bbx = r.x;
        *bby = r.y;
        *bbw = r.width;
        *bbh = r.height;
    }
}
