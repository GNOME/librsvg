extern crate libc;
extern crate cairo;
extern crate cairo_sys;
extern crate glib;

use self::glib::translate::*;

use aspect_ratio::*;
use length::*;

use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use node::*;
use paint_server::*;

use bbox::*;
use util::*;
use viewbox::*;

use self::cairo::MatrixTrait;
use self::cairo::enums::*;
use self::cairo::SurfacePattern;
use self::cairo::Pattern as CairoPattern;

pub struct Pattern {
    pub units:                 Option<PaintServerUnits>,
    pub obj_cbbox:             Option<bool>,
    pub vbox:                  Option<RsvgViewBox>,
    pub preserve_aspect_ratio: Option<AspectRatio>,
    pub affine:                Option<cairo::Matrix>,
    pub fallback:              Option<String>,
    pub x:                     Option<RsvgLength>,
    pub y:                     Option<RsvgLength>,
    pub width:                 Option<RsvgLength>,
    pub height:                Option<RsvgLength>,

    // We just use c_node to see if the C implementation has children
    pub c_node:                *const RsvgNode
}

// A pattern's patternUnits attribute (in our Pattern::units field) defines the coordinate
// system relative to the x/y/width/height of the Pattern.  However, patterns also
// have a patternContentUnits attribute, which refers to the pattern's contents (i.e. the
// objects which it references.  We define PatternContentUnits as a newtype, so that
// it can have its own default value, different from the one in PaintServerUnits.
struct PatternContentUnits(PaintServerUnits);

impl From<PaintServerUnits> for PatternContentUnits {
    fn from (units: PaintServerUnits) -> PatternContentUnits {
        PatternContentUnits(units)
    }
}

impl Default for PatternContentUnits {
    fn default () -> PatternContentUnits {
        PatternContentUnits (PaintServerUnits::UserSpaceOnUse)
    }
}

extern "C" {
    fn rsvg_pattern_node_to_rust_pattern (node: *const RsvgNode) -> *mut Pattern;
}

fn pattern_node_has_children (raw_node: *const RsvgNode) -> bool {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    if node.get_type () == NodeType::Pattern {
        node.children.borrow ().len () > 0
    } else {
        false
    }
}

// All of the Pattern's fields are Option<foo> values, because
// those fields can be omitted in the SVG file.  We need to resolve
// them to default values, or to fallback values that come from
// another Pattern.
//
// For the fallback case, this would need something like
//
//    if self.foo.is_none () { self.foo = fallback.foo; }
//
// And for the default case, it would be like
//    if self.foo.is_none () { self.foo = Some (default_value); }
//
// Both can be replaced by
//
//    self.foo = self.foo.take ().or (bar);
//
// So we define a macro for that.
macro_rules! fallback_to (
    ($dest:expr, $default:expr) => (
        $dest = $dest.take ().or ($default)
    );
);

impl Pattern {
    fn is_resolved (&self) -> bool {
        self.units.is_some () &&
            self.obj_cbbox.is_some () &&
            self.vbox.is_some () &&
            self.preserve_aspect_ratio.is_some () &&
            self.affine.is_some () &&
            self.x.is_some () &&
            self.y.is_some () &&
            self.width.is_some () &&
            self.height.is_some () &&
            pattern_node_has_children (self.c_node)
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        fallback_to! (self.units,                 Some (PaintServerUnits::default ()));
        fallback_to! (self.obj_cbbox,             Some (false));
        fallback_to! (self.vbox,                  Some (RsvgViewBox::new_inactive ()));
        fallback_to! (self.preserve_aspect_ratio, Some (AspectRatio::default ()));
        fallback_to! (self.affine,                Some (cairo::Matrix::identity ()));

        fallback_to! (self.x,                     Some (RsvgLength::default ()));
        fallback_to! (self.y,                     Some (RsvgLength::default ()));
        fallback_to! (self.width,                 Some (RsvgLength::default ()));
        fallback_to! (self.height,                Some (RsvgLength::default ()));

        self.fallback = None;

        // We don't resolve the children here - instead, we'll just
        // NOP if there are no children at drawing time.
    }

    fn resolve_from_fallback (&mut self, fallback: &Pattern) {
        fallback_to! (self.units,                 fallback.units);
        fallback_to! (self.obj_cbbox,             fallback.obj_cbbox);
        fallback_to! (self.vbox,                  fallback.vbox);
        fallback_to! (self.preserve_aspect_ratio, fallback.preserve_aspect_ratio);
        fallback_to! (self.affine,                fallback.affine);
        fallback_to! (self.x,                     fallback.x);
        fallback_to! (self.y,                     fallback.y);
        fallback_to! (self.width,                 fallback.width);
        fallback_to! (self.height,                fallback.height);

        self.fallback = clone_fallback_name (&fallback.fallback);

        if !pattern_node_has_children (self.c_node) {
            self.c_node = fallback.c_node;
        }
    }
}

impl Clone for Pattern {
    fn clone (&self) -> Self {
        Pattern {
            units:                 self.units,
            obj_cbbox:             self.obj_cbbox,
            vbox:                  self.vbox,
            preserve_aspect_ratio: self.preserve_aspect_ratio,
            affine:                self.affine,
            fallback:              clone_fallback_name (&self.fallback),
            x:                     self.x,
            y:                     self.y,
            width:                 self.width,
            height:                self.height,
            c_node:                self.c_node
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

fn paint_server_units_from_bool (v: bool) -> PaintServerUnits {
    if v {
        PaintServerUnits::ObjectBoundingBox
    } else {
        PaintServerUnits::UserSpaceOnUse
    }
}

fn set_pattern_on_draw_context (pattern: &Pattern,
                                draw_ctx: *mut RsvgDrawingCtx,
                                bbox:     &RsvgBbox) -> bool {
    assert! (pattern.is_resolved ());

    if !pattern_node_has_children (pattern.c_node) {
        return false;
    }

    let units                 = pattern.units.unwrap ();
    let obj_cbbox             = pattern.obj_cbbox.unwrap ();
    let pattern_affine        = pattern.affine.unwrap ();
    let vbox                  = pattern.vbox.unwrap ();
    let preserve_aspect_ratio = pattern.preserve_aspect_ratio.unwrap ();

    if units == PaintServerUnits::ObjectBoundingBox {
        drawing_ctx::push_view_box (draw_ctx, 1.0, 1.0);
    }

    let pattern_x      = pattern.x.unwrap ().normalize (draw_ctx);
    let pattern_y      = pattern.y.unwrap ().normalize (draw_ctx);
    let pattern_width  = pattern.width.unwrap ().normalize (draw_ctx);
    let pattern_height = pattern.height.unwrap ().normalize (draw_ctx);

    if units == PaintServerUnits::ObjectBoundingBox {
        drawing_ctx::pop_view_box (draw_ctx);
    }

    // Work out the size of the rectangle so it takes into account the object bounding box

    let bbwscale: f64;
    let bbhscale: f64;

    match units {
        PaintServerUnits::ObjectBoundingBox => {
            bbwscale = bbox.rect.width;
            bbhscale = bbox.rect.height;
        },

        PaintServerUnits::UserSpaceOnUse => {
            bbwscale = 1.0;
            bbhscale = 1.0;
        }
    }

    let taffine = cairo::Matrix::multiply (&pattern_affine, &drawing_ctx::get_current_state_affine (draw_ctx));

    let mut scwscale = (taffine.xx * taffine.xx + taffine.xy * taffine.xy).sqrt ();
    let mut schscale = (taffine.yx * taffine.yx + taffine.yy * taffine.yy).sqrt ();

    let pw: i32 = (pattern_width * bbwscale * scwscale) as i32;
    let ph: i32 = (pattern_height * bbhscale * schscale) as i32;

    let scaled_width = pattern_width * bbwscale;
    let scaled_height = pattern_height * bbhscale;

    if scaled_width.abs () < DBL_EPSILON || scaled_height.abs () < DBL_EPSILON
        || pw < 1 || ph < 1 {
        return false;
    }

    scwscale = pw as f64 / scaled_width;
    schscale = ph as f64 / scaled_height;

    let mut affine: cairo::Matrix = cairo::Matrix::identity ();

    // Create the pattern coordinate system
    match units {
        PaintServerUnits::ObjectBoundingBox => {
            affine.translate (bbox.rect.x + pattern_x * bbox.rect.width,
                              bbox.rect.y + pattern_y * bbox.rect.height);
        },

        PaintServerUnits::UserSpaceOnUse => {
            affine.translate (pattern_x, pattern_y);
        }
    }

    // Apply the pattern transform
    affine = cairo::Matrix::multiply (&affine, &pattern_affine);

    let mut caffine: cairo::Matrix;

    let pushed_view_box: bool;

        // Create the pattern contents coordinate system
    if vbox.active {
        // If there is a vbox, use that
        let (mut x, mut y, w, h) = preserve_aspect_ratio.compute (vbox.rect.width,
                                                                  vbox.rect.height,
                                                                  0.0,
                                                                  0.0,
                                                                  pattern_width * bbwscale,
                                                                  pattern_height * bbhscale);

        x -= vbox.rect.x * w / vbox.rect.width;
        y -= vbox.rect.y * h / vbox.rect.height;

        caffine = cairo::Matrix::new (w / vbox.rect.width,
                                      0.0,
                                      0.0,
                                      h / vbox.rect.height,
                                      x,
                                      y);

        drawing_ctx::push_view_box (draw_ctx, vbox.rect.width, vbox.rect.height);
        pushed_view_box = true;
    } else if obj_cbbox {
        // If coords are in terms of the bounding box, use them

        caffine = cairo::Matrix::identity ();
        caffine.scale (bbox.rect.width, bbox.rect.height);

        drawing_ctx::push_view_box (draw_ctx, 1.0, 1.0);
        pushed_view_box = true;
    } else {
        caffine = cairo::Matrix::identity ();
        pushed_view_box = false;
    }

    if scwscale != 1.0 || schscale != 1.0 {
        let mut scalematrix = cairo::Matrix::identity ();
        scalematrix.scale (scwscale, schscale);
        caffine = cairo::Matrix::multiply (&caffine, &scalematrix);

        scalematrix = cairo::Matrix::identity ();
        scalematrix.scale (1.0 / scwscale, 1.0 / schscale);

        affine = cairo::Matrix::multiply (&scalematrix, &affine);
    }

    // Draw to another surface

    let cr_save = drawing_ctx::get_cairo_context (draw_ctx);
    drawing_ctx::state_push (draw_ctx);

    let surface = cr_save.get_target ().create_similar (Content::ColorAlpha, pw, ph);

    let cr_pattern = cairo::Context::new (&surface);

    drawing_ctx::set_cairo_context (draw_ctx, &cr_pattern);

    // Set up transformations to be determined by the contents units
    drawing_ctx::set_current_state_affine (draw_ctx, caffine);

    // Draw everything
    let pattern_node: &RsvgNode = unsafe { & *pattern.c_node };
    pattern_node.draw_children (draw_ctx, 2);

    // Return to the original coordinate system and rendering context

    drawing_ctx::state_pop (draw_ctx);
    drawing_ctx::set_cairo_context (draw_ctx, &cr_save);

    if pushed_view_box {
        drawing_ctx::pop_view_box (draw_ctx);
    }

    // Set the final surface as a Cairo pattern into the Cairo context

    let surface_pattern = SurfacePattern::create (&surface);
    surface_pattern.set_extend (Extend::Repeat);

    let mut matrix = affine;
    matrix.invert ();

    surface_pattern.set_matrix (matrix);
    surface_pattern.set_filter (Filter::Best);

    cr_save.set_source (&surface_pattern);

    true
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
                                  fallback_name: *const libc::c_char,
                                  c_node: *const RsvgNode) -> *mut Pattern {
    assert! (!c_node.is_null ());

    let my_x         = { if x.is_null ()      { None } else { Some (*x) } };
    let my_y         = { if y.is_null ()      { None } else { Some (*y) } };
    let my_width     = { if width.is_null ()  { None } else { Some (*width) } };
    let my_height    = { if height.is_null () { None } else { Some (*height) } };

    let my_units     = { if obj_bbox.is_null ()  { None } else { Some (paint_server_units_from_bool (*obj_bbox)) } };
    let my_obj_cbbox = { if obj_cbbox.is_null () { None } else { Some (*obj_cbbox) } };
    let my_vbox      = { if vbox.is_null ()      { None } else { Some (*vbox) } };

    let my_affine    = { if affine.is_null () { None } else { Some (*affine) } };

    let my_preserve_aspect_ratio = { if preserve_aspect_ratio.is_null () { None } else { Some (AspectRatio::from_u32 (*preserve_aspect_ratio)) } };

    let my_fallback_name = { if fallback_name.is_null () { None } else { Some (String::from_glib_none (fallback_name)) } };

    let pattern = Pattern {
        units:                 my_units,
        obj_cbbox:             my_obj_cbbox,
        vbox:                  my_vbox,
        preserve_aspect_ratio: my_preserve_aspect_ratio,
        affine:                my_affine,
        fallback:              my_fallback_name,
        x:                     my_x,
        y:                     my_y,
        width:                 my_width,
        height:                my_height,
        c_node:                c_node
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
                                                         bbox:        RsvgBbox) -> bool {
    assert! (!raw_pattern.is_null ());
    let pattern: &mut Pattern = unsafe { &mut (*raw_pattern) };

    let mut fallback_source = NodeFallbackSource::new (draw_ctx);

    let resolved = resolve_pattern (pattern, &mut fallback_source);

    set_pattern_on_draw_context (&resolved,
                                 draw_ctx,
                                 &bbox)
}
