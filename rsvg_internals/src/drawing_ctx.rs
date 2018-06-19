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
use defs::{self, RsvgDefs};
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

    fn rsvg_drawing_ctx_add_node_to_stack(draw_ctx: *const RsvgDrawingCtx, node: *const RsvgNode);

    fn rsvg_drawing_ctx_prepend_acquired_node(
        draw_ctx: *const RsvgDrawingCtx,
        node: *mut RsvgNode,
    ) -> glib_sys::gboolean;

    fn rsvg_drawing_ctx_remove_acquired_node(draw_ctx: *const RsvgDrawingCtx, node: *mut RsvgNode);

    fn rsvg_drawing_ctx_get_defs(draw_ctx: *const RsvgDrawingCtx) -> *const RsvgDefs;

    fn rsvg_drawing_ctx_get_offset(
        draw_ctx: *const RsvgDrawingCtx,
        out_x: *mut f64,
        out_y: *mut f64,
    );

    fn rsvg_drawing_ctx_get_raw_offset(
        draw_ctx: *const RsvgDrawingCtx,
        out_x: *mut f64,
        out_y: *mut f64,
    );

    fn rsvg_drawing_ctx_get_bbox(draw_ctx: *const RsvgDrawingCtx) -> *mut RsvgBbox;

    fn rsvg_drawing_ctx_get_cr_stack(draw_ctx: *mut RsvgDrawingCtx) -> *mut glib_sys::GList;

    fn rsvg_drawing_ctx_is_cairo_context_nested(
        draw_ctx: *const RsvgDrawingCtx,
        cr: *mut cairo_sys::cairo_t,
    ) -> glib_sys::gboolean;

    fn rsvg_drawing_ctx_is_testing(draw_ctx: *const RsvgDrawingCtx) -> glib_sys::gboolean;

    fn rsvg_drawing_ctx_draw_node_on_surface(
        draw_ctx: *mut RsvgDrawingCtx,
        node: *const RsvgNode,
        cascade_from: *const RsvgNode,
        surface: *mut cairo_sys::cairo_surface_t,
        width: f64,
        height: f64,
    );
}

pub fn get_cairo_context(draw_ctx: *const RsvgDrawingCtx) -> cairo::Context {
    unsafe {
        let raw_cr = rsvg_drawing_ctx_get_cairo_context(draw_ctx);

        cairo::Context::from_glib_none(raw_cr)
    }
}

// FIXME: Usage of this function is more less a hack... The caller
// manually saves and then restore the draw_ctx.cr.
// It would be better to have an explicit push/pop for the cairo_t, or
// pushing a temporary surface, or something that does not involve
// monkeypatching the cr directly.
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

// Use this function when looking up urls to other nodes. This function
// does proper recursion checking and thereby avoids infinite loops.
//
// Nodes acquired by this function must be released in reverse
// acquiring order.
//
// Note that if you acquire a node, you have to release it before trying to
// acquire it again.  If you acquire a node "#foo" and don't release it before
// trying to acquire "foo" again, you will obtain a %NULL the second time.
pub fn get_acquired_node(draw_ctx: *const RsvgDrawingCtx, url: &str) -> Option<AcquiredNode> {
    let defs = unsafe {
        let d = rsvg_drawing_ctx_get_defs(draw_ctx);
        &*d
    };

    if let Some(node) = defs::lookup(defs, url) {
        unsafe {
            if from_glib(rsvg_drawing_ctx_prepend_acquired_node(draw_ctx, node)) {
                return Some(AcquiredNode(draw_ctx, node));
            }
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
pub fn get_acquired_node_of_type(
    draw_ctx: *const RsvgDrawingCtx,
    url: Option<&str>,
    node_type: NodeType,
) -> Option<AcquiredNode> {
    url.and_then(|url| get_acquired_node(draw_ctx, url))
        .and_then(|acquired| {
            if acquired.get().get_type() == node_type {
                Some(acquired)
            } else {
                None
            }
        })
}

pub fn with_discrete_layer(
    draw_ctx: *mut RsvgDrawingCtx,
    node: &RsvgNode,
    values: &ComputedValues,
    clipping: bool,
    draw_fn: &mut FnMut(&cairo::Context),
) {
    if clipping {
        draw_fn(&get_cairo_context(draw_ctx));
    } else {
        let original_cr = get_cairo_context(draw_ctx);
        original_cr.save();

        let clip_uri = match values.clip_path {
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

        let current_affine = original_cr.get_matrix();

        let (clip_node, clip_units) = {
            let clip_node = get_acquired_node_of_type(
                draw_ctx,
                clip_uri.map(String::as_ref),
                NodeType::ClipPath,
            ).and_then(|acquired| Some(acquired.get()));

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
                    clip_path.to_cairo_context(clip_node, &current_affine, draw_ctx);
                });
            }
        }

        let needs_temporary_surface = !(opacity == 1.0
            && filter.is_none()
            && mask.is_none()
            && (clip_units == None || clip_units == Some(CoordUnits::UserSpaceOnUse))
            && comp_op == CompOp::SrcOver
            && enable_background == EnableBackground::Accumulate);

        let (child_surface, child_cr) = {
            if needs_temporary_surface {
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
                child_cr.set_matrix(original_cr.get_matrix());

                unsafe {
                    rsvg_drawing_ctx_push_cr(draw_ctx, child_cr.to_raw_none());
                }

                unsafe {
                    rsvg_drawing_ctx_push_bounding_box(draw_ctx);
                }

                (surface, child_cr)
            } else {
                (
                    cairo::ImageSurface::from(original_cr.get_target()).unwrap(),
                    original_cr.clone(),
                )
            }
        };

        draw_fn(&child_cr);

        if needs_temporary_surface {
            let filter_result_surface = filter
                .and_then(|_| {
                    // About the following unwrap(), see the FIXME above.  We should be pushing
                    // only surfaces that are not in an error state, but currently we don't
                    // actually ensure that.
                    let output = unsafe {
                        cairo::ImageSurface::from_raw_full(rsvg_drawing_ctx_pop_surface(draw_ctx))
                            .unwrap()
                    };

                    // The bbox rect can be None, for example, if a filter is applied to an empty
                    // group. There's nothing to filter in this case.
                    get_bbox(draw_ctx).rect.and_then(|_| {
                        get_acquired_node_of_type(
                            draw_ctx,
                            filter.map(String::as_ref),
                            NodeType::Filter,
                        ).and_then(|acquired| {
                            let filter_node = acquired.get();

                            if !filter_node.is_in_error() {
                                // FIXME: deal with out of memory here
                                Some(filter_render(
                                    &filter_node,
                                    node,
                                    &output,
                                    draw_ctx,
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

            unsafe {
                rsvg_drawing_ctx_pop_cr(draw_ctx);
            }

            let current_affine = original_cr.get_matrix();

            let (xofs, yofs) = get_offset(draw_ctx);

            original_cr.identity_matrix();
            original_cr.set_source_surface(&filter_result_surface, xofs, yofs);

            if clip_units == Some(CoordUnits::ObjectBoundingBox) {
                if let Some(ref clip_node) = clip_node {
                    clip_node.with_impl(|clip_path: &NodeClipPath| {
                        clip_path.to_cairo_context(clip_node, &current_affine, draw_ctx);
                    });
                }
            }

            original_cr.set_operator(cairo::Operator::from(comp_op));

            if let Some(mask) = mask {
                if let Some(acquired) =
                    get_acquired_node_of_type(draw_ctx, Some(mask), NodeType::Mask)
                {
                    let node = acquired.get();

                    node.with_impl(|mask: &NodeMask| {
                        mask.generate_cairo_mask(&node, &current_affine, draw_ctx);
                    });
                }
            } else if opacity < 1.0 {
                original_cr.paint_with_alpha(opacity);
            } else {
                original_cr.paint();
            }

            unsafe {
                rsvg_drawing_ctx_pop_bounding_box(draw_ctx);
            }
        }

        original_cr.restore();
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

pub fn get_raw_offset(draw_ctx: *const RsvgDrawingCtx) -> (f64, f64) {
    let mut w: f64 = 0.0;
    let mut h: f64 = 0.0;

    unsafe {
        rsvg_drawing_ctx_get_raw_offset(draw_ctx, &mut w, &mut h);
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

pub fn get_cr_stack(draw_ctx: *mut RsvgDrawingCtx) -> Vec<cairo::Context> {
    let mut res = Vec::new();

    unsafe {
        let list = rsvg_drawing_ctx_get_cr_stack(draw_ctx);

        let mut list = glib_sys::g_list_first(mut_override(list));
        while !list.is_null() {
            res.push(from_glib_none((*list).data as *mut cairo_sys::cairo_t));
            list = (*list).next;
        }
    }

    res
}

pub fn is_cairo_context_nested(draw_ctx: *const RsvgDrawingCtx, cr: &cairo::Context) -> bool {
    let cr = cr.to_glib_none();
    from_glib(unsafe { rsvg_drawing_ctx_is_cairo_context_nested(draw_ctx, cr.0) })
}

pub fn draw_node_on_surface(
    draw_ctx: *mut RsvgDrawingCtx,
    node: &RsvgNode,
    cascade_from: &RsvgNode,
    surface: &cairo::ImageSurface,
    width: f64,
    height: f64,
) {
    unsafe {
        rsvg_drawing_ctx_draw_node_on_surface(
            draw_ctx,
            node,
            cascade_from,
            surface.to_glib_none().0,
            width,
            height,
        );
    }
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

#[allow(improper_ctypes)]
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
                node.draw(node, cascaded, draw_ctx, clipping);
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

    draw_node_from_stack(draw_ctx, &cascaded, node, clipping);
}

fn add_node_and_ancestors_to_stack(draw_ctx: *const RsvgDrawingCtx, node: &RsvgNode) {
    unsafe {
        rsvg_drawing_ctx_add_node_to_stack(draw_ctx, node);
    }

    if let Some(ref parent) = node.get_parent() {
        add_node_and_ancestors_to_stack(draw_ctx, parent);
    }
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_add_node_and_ancestors_to_stack(
    draw_ctx: *const RsvgDrawingCtx,
    raw_node: *const RsvgNode,
) {
    assert!(!draw_ctx.is_null());

    assert!(!raw_node.is_null());
    let node = unsafe { &*raw_node };

    add_node_and_ancestors_to_stack(draw_ctx, node);
}

#[no_mangle]
pub extern "C" fn rsvg_drawing_ctx_get_ink_rect(
    draw_ctx: *const RsvgDrawingCtx,
    ink_rect: *mut cairo_sys::cairo_rectangle_t,
) {
    assert!(!draw_ctx.is_null());
    assert!(!ink_rect.is_null());

    let r = get_bbox(draw_ctx).ink_rect.unwrap();
    unsafe {
        (*ink_rect).x = r.x;
        (*ink_rect).y = r.y;
        (*ink_rect).width = r.width;
        (*ink_rect).height = r.height;
    }
}

pub struct AcquiredNode(*const RsvgDrawingCtx, *mut RsvgNode);

impl Drop for AcquiredNode {
    fn drop(&mut self) {
        unsafe {
            rsvg_drawing_ctx_remove_acquired_node(self.0, self.1);
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
