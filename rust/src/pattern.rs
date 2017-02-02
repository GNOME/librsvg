extern crate libc;
extern crate cairo;
extern crate cairo_sys;
extern crate glib;

use self::glib::translate::*;

use length::*;

use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use drawing_ctx::RsvgNode;

use bbox::*;
use util::*;
use viewbox::*;

use self::cairo::MatrixTrait;
use self::cairo::enums::Content;

pub struct Pattern {
    pub obj_bbox:              Option<bool>,
    pub obj_cbbox:             Option<bool>,
    pub vbox:                  Option<RsvgViewBox>,
    pub preserve_aspect_ratio: Option<u32>,
    pub affine:                Option<cairo::Matrix>,
    pub fallback:              Option<String>,
    pub x:                     Option<RsvgLength>,
    pub y:                     Option<RsvgLength>,
    pub width:                 Option<RsvgLength>,
    pub height:                Option<RsvgLength>
}

impl Pattern {
    fn is_resolved (&self) -> bool {
        self.obj_bbox.is_some () &&
            self.obj_cbbox.is_some () &&
            self.vbox.is_some () &&
            self.preserve_aspect_ratio.is_some () &&
            self.affine.is_some () &&
            self.x.is_some () &&
            self.y.is_some () &&
            self.width.is_some () &&
            self.height.is_some ()
        // FIXME: which fallback contains the children?
    }

    fn resolve_from_defaults (&mut self) {
        /* FIXME: check the spec */
        /* These are per the spec */

        if self.obj_bbox.is_none ()  { self.obj_bbox  = Some (true); }
        if self.obj_cbbox.is_none () { self.obj_cbbox = Some (false); }
        if self.vbox.is_none ()      { self.vbox      = Some (RsvgViewBox::new_inactive ()); }

        // FIXME: this is RSVG_ASPECT_RATIO_XMID_YMID; use a constant, not a number.  Spec says "xMidYMid meet"
        if self.preserve_aspect_ratio.is_none () { self.preserve_aspect_ratio = Some (1 << 4); }

        if self.affine.is_none ()    { self.affine    = Some (cairo::Matrix::identity ()); }

        self.fallback = None;

        if self.x.is_none ()         { self.x         = Some (RsvgLength::parse ("0", LengthDir::Horizontal)); }
        if self.y.is_none ()         { self.y         = Some (RsvgLength::parse ("0", LengthDir::Horizontal)); }
        if self.width.is_none ()     { self.width     = Some (RsvgLength::parse ("0", LengthDir::Horizontal)); }
        if self.height.is_none ()    { self.height    = Some (RsvgLength::parse ("0", LengthDir::Horizontal)); }
    }

    fn resolve_from_fallback (&mut self, fallback: &Pattern) {
        if self.obj_bbox.is_none ()  { self.obj_bbox  = fallback.obj_bbox; }
        if self.obj_cbbox.is_none () { self.obj_cbbox = fallback.obj_cbbox; }
        if self.vbox.is_none ()      { self.vbox      = fallback.vbox; }

        if self.preserve_aspect_ratio.is_none () { self.preserve_aspect_ratio = fallback.preserve_aspect_ratio; }

        if self.affine.is_none ()    { self.affine    = fallback.affine; }

        if self.x.is_none ()         { self.x         = fallback.x; }
        if self.y.is_none ()         { self.y         = fallback.y; }
        if self.width.is_none ()     { self.width     = fallback.width; }
        if self.height.is_none ()    { self.height    = fallback.height; }

        if self.fallback.is_none () {
            self.fallback = clone_fallback_name (&fallback.fallback);
        }
    }
}

impl Clone for Pattern {
    fn clone (&self) -> Self {
        Pattern {
            obj_bbox:              self.obj_bbox,
            obj_cbbox:             self.obj_cbbox,
            vbox:                  self.vbox,
            preserve_aspect_ratio: self.preserve_aspect_ratio,
            affine:                self.affine,
            fallback:              clone_fallback_name (&self.fallback),
            x:                     self.x,
            y:                     self.y,
            width:                 self.width,
            height:                self.height,
        }
    }
}

trait FallbackSource {
    fn get_fallback (&mut self, name: &str) -> Option<Box<Pattern>>;
}

fn resolve_pattern (pattern: &Pattern, fallback_source: &mut FallbackSource) -> Pattern {
    let mut result = pattern.clone ();

    while !result.is_resolved () {
        let mut opt_fallback: Option<Box<Pattern>> = None;

        if let Some (ref fallback_name) = result.fallback {
            opt_fallback = fallback_source.get_fallback (&**fallback_name);
        }

        if let Some (fallback_pattern) = opt_fallback {
            result.resolve_from_fallback (&*fallback_pattern);
        } else {
            result.resolve_from_defaults ();
            break;
        }
    }

    result
}

struct NodeFallbackSource {
    draw_ctx: *mut RsvgDrawingCtx,
    acquired_nodes: Vec<*mut RsvgNode>
}

impl NodeFallbackSource {
    fn new (draw_ctx: *mut RsvgDrawingCtx) -> NodeFallbackSource {
        NodeFallbackSource {
            draw_ctx: draw_ctx,
            acquired_nodes: Vec::<*mut RsvgNode>::new ()
        }
    }
}

impl Drop for NodeFallbackSource {
    fn drop (&mut self) {
        while let Some (node) = self.acquired_nodes.pop () {
            drawing_ctx::release_node (self.draw_ctx, node);
        }
    }
}

extern "C" {
    fn rsvg_pattern_node_to_rust_pattern (node: *const RsvgNode) -> *mut Pattern;
}

impl FallbackSource for NodeFallbackSource {
    fn get_fallback (&mut self, name: &str) -> Option<Box<Pattern>> {
        let fallback_node = drawing_ctx::acquire_node (self.draw_ctx, name);

        if fallback_node.is_null () {
            return None;
        }

        self.acquired_nodes.push (fallback_node);

        let raw_fallback_pattern = unsafe { rsvg_pattern_node_to_rust_pattern (fallback_node) };

        if raw_fallback_pattern.is_null () {
            return None;
        }

        let fallback_pattern = unsafe { Box::from_raw (raw_fallback_pattern) };

        return Some (fallback_pattern);
    }
}

fn set_pattern_on_draw_context (pattern: &Pattern,
                                draw_ctx: *mut RsvgDrawingCtx,
                                opacity:  u8,
                                bbox:     &RsvgBbox) {
    assert! (pattern.is_resolved ());

    let obj_bbox = pattern.obj_bbox.unwrap ();

    if obj_bbox {
        drawing_ctx::push_view_box (draw_ctx, 1.0, 1.0);
    }

    let pattern_x      = pattern.x.unwrap ().normalize (draw_ctx);
    let pattern_y      = pattern.y.unwrap ().normalize (draw_ctx);
    let pattern_width  = pattern.width.unwrap ().normalize (draw_ctx);
    let pattern_height = pattern.height.unwrap ().normalize (draw_ctx);

    if obj_bbox {
        drawing_ctx::pop_view_box (draw_ctx);
    }

    // Work out the size of the rectangle so it takes into account the object bounding box

    let bbwscale: f64;
    let bbhscale: f64;

    if obj_bbox {
        bbwscale = bbox.rect.width;
        bbhscale = bbox.rect.height;
    } else {
        bbwscale = 1.0;
        bbhscale = 1.0;
    }

    let taffine = cairo::Matrix::multiply (&pattern.affine.unwrap (), &drawing_ctx::get_current_state_affine (draw_ctx));

    let mut scwscale = (taffine.xx * taffine.xx + taffine.xy * taffine.xy).sqrt ();
    let mut schscale = (taffine.yx * taffine.yx + taffine.yy * taffine.yy).sqrt ();

    let pw = pattern_width * bbwscale * scwscale;
    let ph = pattern_height * bbhscale * schscale;

    let scaled_width = pattern_width * bbwscale;
    let scaled_height = pattern_height * bbhscale;

    if scaled_width.abs () < DBL_EPSILON || scaled_height.abs () < DBL_EPSILON {
        return
    }

    scwscale = pw / scaled_width;
    schscale = ph / scaled_height;

    let cr = drawing_ctx::get_cairo_context (draw_ctx);

    let surface = cr.get_target ().create_similar (Content::ColorAlpha, pw as i32, ph as i32);

    let cr_pattern = cairo::Context::new (&surface);

    let mut affine: cairo::Matrix = cairo::Matrix::identity ();

    // Create the pattern coordinate system
    if obj_bbox {
        affine.translate (bbox.rect.x + pattern_x * bbox.rect.width,
                          bbox.rect.y + pattern_y * bbox.rect.height);
    } else {
        affine.translate (pattern_x, pattern_y);
    }

    // Apply the pattern transform
    affine = cairo::Matrix::multiply (&affine, pattern.affine.as_ref ().unwrap ());

    // Create the pattern contents coordinate system
    if pattern.vbox.unwrap ().active {
        // If there is a vbox, use that
        let w = pattern_width * bbwscale;
        let h = pattern_height * bbhscale;
        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
    }
    
}

#[no_mangle]
pub unsafe extern fn pattern_new (x: *const RsvgLength,
                                  y: *const RsvgLength,
                                  width: *const RsvgLength,
                                  height: *const RsvgLength,
                                  obj_bbox: *const bool,
                                  obj_cbbox: *const bool,
                                  vbox: *const RsvgViewBox,
                                  affine: *const cairo::Matrix,
                                  preserve_aspect_ratio: *const u32,
                                  fallback_name: *const libc::c_char) -> *mut Pattern {
    let my_x         = { if x.is_null ()      { None } else { Some (*x) } };
    let my_y         = { if y.is_null ()      { None } else { Some (*y) } };
    let my_width     = { if width.is_null ()  { None } else { Some (*width) } };
    let my_height    = { if height.is_null () { None } else { Some (*height) } };

    let my_obj_bbox  = { if obj_bbox.is_null ()  { None } else { Some (*obj_bbox) } };
    let my_obj_cbbox = { if obj_cbbox.is_null () { None } else { Some (*obj_cbbox) } };
    let my_vbox      = { if vbox.is_null ()      { None } else { Some (*vbox) } };

    let my_affine    = { if affine.is_null () { None } else { Some (*affine) } };

    let my_preserve_aspect_ratio = { if preserve_aspect_ratio.is_null () { None } else { Some (*preserve_aspect_ratio) } };

    let my_fallback_name = { if fallback_name.is_null () { None } else { Some (String::from_glib_none (fallback_name)) } };

    let pattern = Pattern {
        obj_bbox:              my_obj_bbox,
        obj_cbbox:             my_obj_cbbox,
        vbox:                  my_vbox,
        preserve_aspect_ratio: my_preserve_aspect_ratio,
        affine:                my_affine,
        fallback:              my_fallback_name,
        x:                     my_x,
        y:                     my_y,
        width:                 my_width,
        height:                my_height
    };

    let boxed_pattern = Box::new (pattern);

    Box::into_raw (boxed_pattern)
}

#[no_mangle]
pub unsafe extern fn pattern_destroy (raw_pattern: *mut Pattern) {
    assert! (!raw_pattern.is_null ());

    let _ = Box::from_raw (raw_pattern);
}

#[no_mangle]
pub extern fn pattern_resolve_fallbacks_and_set_pattern (raw_pattern: *mut Pattern,
                                                         draw_ctx:    *mut RsvgDrawingCtx,
                                                         opacity:     u8,
                                                         bbox:        RsvgBbox) {
    assert! (!raw_pattern.is_null ());
    let pattern: &mut Pattern = unsafe { &mut (*raw_pattern) };

    let mut fallback_source = NodeFallbackSource::new (draw_ctx);

    let resolved = resolve_pattern (pattern, &mut fallback_source);

    set_pattern_on_draw_context (&resolved,
                                 draw_ctx,
                                 opacity,
                                 &bbox);
}
