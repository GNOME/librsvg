use ::cairo;
use ::glib_sys;
use ::glib::translate::*;
use ::libc;

use std::cell::RefCell;
use std::rc::*;

use cairo::MatrixTrait;
use cairo::Pattern as CairoPattern;

use aspect_ratio::*;
use bbox::*;
use drawing_ctx;
use drawing_ctx::RsvgDrawingCtx;
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use paint_server::*;
use parsers::Parse;
use property_bag;
use property_bag::*;
use util::*;
use viewbox::*;

#[derive(Clone)]
pub struct Pattern {
    pub units:                 Option<PaintServerUnits>,
    pub content_units:         Option<PatternContentUnits>,
    // This Option<Option<ViewBox>> is a bit strange.  We want a field
    // with value None to mean, "this field isn't resolved yet".  However,
    // the vbox can very well be *not* specified in the SVG file.
    // In that case, the fully resolved pattern will have a .vbox=Some(None) value.
    pub vbox:                  Option<Option<ViewBox>>,
    pub preserve_aspect_ratio: Option<AspectRatio>,
    pub affine:                Option<cairo::Matrix>,
    pub fallback:              Option<String>,
    pub x:                     Option<RsvgLength>,
    pub y:                     Option<RsvgLength>,
    pub width:                 Option<RsvgLength>,
    pub height:                Option<RsvgLength>,

    // Point back to our corresponding node, or to the fallback node which has children
    pub node:                  Option<Weak<Node>>
}

impl Default for Pattern {
    fn default () -> Pattern {
        Pattern {
            units:                 None,
            content_units:         None,
            vbox:                  None,
            preserve_aspect_ratio: None,
            affine:                None,
            fallback:              None,
            x:                     None,
            y:                     None,
            width:                 None,
            height:                None,
            node:                  None
        }
    }
}

// A pattern's patternUnits attribute (in our Pattern::units field) defines the coordinate
// system relative to the x/y/width/height of the Pattern.  However, patterns also
// have a patternContentUnits attribute, which refers to the pattern's contents (i.e. the
// objects which it references.  We define PatternContentUnits as a newtype, so that
// it can have its own default value, different from the one in PaintServerUnits.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PatternContentUnits(PaintServerUnits);

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

impl Parse for PatternContentUnits {
    type Data = ();
    type Err = AttributeError;

    fn parse (s: &str, _: ()) -> Result<PatternContentUnits, AttributeError> {
        Ok (PatternContentUnits::from (PaintServerUnits::parse (s, ())?))
    }
}

fn node_has_children (node: &Option<Weak<Node>>) -> bool {
    match *node {
        None => false,

        Some (ref weak) => {
            let ref strong_node = weak.clone ().upgrade ().unwrap ();
            let has_children = strong_node.children.borrow ().len () > 0;
            has_children
        }
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
            self.content_units.is_some () &&
            self.vbox.is_some () &&
            self.preserve_aspect_ratio.is_some () &&
            self.affine.is_some () &&
            self.x.is_some () &&
            self.y.is_some () &&
            self.width.is_some () &&
            self.height.is_some () &&
            node_has_children (&self.node)
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        fallback_to! (self.units,                 Some (PaintServerUnits::default ()));
        fallback_to! (self.content_units,         Some (PatternContentUnits::default ()));
        fallback_to! (self.vbox,                  Some (None));
        fallback_to! (self.preserve_aspect_ratio, Some (AspectRatio::default ()));
        fallback_to! (self.affine,                Some (cairo::Matrix::identity ()));

        fallback_to! (self.x,                     Some (RsvgLength::default ()));
        fallback_to! (self.y,                     Some (RsvgLength::default ()));
        fallback_to! (self.width,                 Some (RsvgLength::default ()));
        fallback_to! (self.height,                Some (RsvgLength::default ()));

        self.fallback = None;

        if !node_has_children (&self.node) {
            self.node = None;
        }
    }

    fn resolve_from_fallback (&mut self, fallback: &Pattern) {
        fallback_to! (self.units,                 fallback.units);
        fallback_to! (self.content_units,         fallback.content_units);
        fallback_to! (self.vbox,                  fallback.vbox);
        fallback_to! (self.preserve_aspect_ratio, fallback.preserve_aspect_ratio);
        fallback_to! (self.affine,                fallback.affine);
        fallback_to! (self.x,                     fallback.x);
        fallback_to! (self.y,                     fallback.y);
        fallback_to! (self.width,                 fallback.width);
        fallback_to! (self.height,                fallback.height);

        self.fallback = fallback.fallback.clone ();

        if !node_has_children (&self.node) {
            self.node = fallback.node.clone ();
        }
    }
}

struct NodePattern {
    pattern: RefCell<Pattern>
}

impl NodePattern {
    fn new () -> NodePattern {
        NodePattern {
            pattern: RefCell::new (Pattern::default ())
        }
    }
}

impl NodeTrait for NodePattern {
    fn set_atts (&self, node: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        let mut p = self.pattern.borrow_mut ();

        p.node = Some (Rc::downgrade (node));

        p.units         = property_bag::parse_or_none (pbag, "patternUnits", ())?;
        p.content_units = property_bag::parse_or_none (pbag, "patternContentUnits", ())?;
        p.vbox          = property_bag::parse_or_none (pbag, "viewBox", ())?.map (Some).or (None);

        p.preserve_aspect_ratio = property_bag::parse_or_none (pbag, "preserveAspectRatio", ())?;

        p.affine = property_bag::parse_or_none (pbag, "patternTransform", ())?;

        p.fallback = property_bag::lookup (pbag, "xlink:href");

        p.x      = property_bag::length_or_none (pbag, "x", LengthDir::Horizontal)?;
        p.y      = property_bag::length_or_none (pbag, "y", LengthDir::Vertical)?;
        p.width  = property_bag::length_or_none (pbag, "width", LengthDir::Horizontal)?;
        p.height = property_bag::length_or_none (pbag, "height", LengthDir::Vertical)?;

        Ok (())
    }

    fn draw (&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing; paint servers are handled specially
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

trait FallbackSource {
    fn get_fallback (&mut self, name: &str) -> Option<RsvgNode>;
}

fn resolve_pattern (pattern: &Pattern, fallback_source: &mut FallbackSource) -> Pattern {
    let mut result = pattern.clone ();

    while !result.is_resolved () {
        let mut opt_fallback: Option<RsvgNode> = None;

        if let Some (ref fallback_name) = result.fallback {
            opt_fallback = fallback_source.get_fallback (&**fallback_name);
        }

        if let Some (fallback_node) = opt_fallback {
            fallback_node.with_impl (|i: &NodePattern|
                                     result.resolve_from_fallback (&*i.pattern.borrow ()));
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
    fn get_fallback (&mut self, name: &str) -> Option<RsvgNode> {
        let raw_fallback_node = drawing_ctx::acquire_node_of_type (self.draw_ctx, name, NodeType::Pattern);

        if raw_fallback_node.is_null () {
            return None;
        }

        self.acquired_nodes.push (raw_fallback_node);

        let fallback_node: &RsvgNode = unsafe { & *raw_fallback_node };

        Some (fallback_node.clone ())
    }
}

fn set_pattern_on_draw_context (pattern: &Pattern,
                                draw_ctx: *mut RsvgDrawingCtx,
                                bbox:     &RsvgBbox) -> bool {
    assert! (pattern.is_resolved ());

    if !node_has_children (&pattern.node) {
        return false;
    }

    let units                 = pattern.units.unwrap ();
    let content_units         = pattern.content_units.unwrap ();
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
    if let Some (vbox) = vbox {
        // If there is a vbox, use that
        let (mut x, mut y, w, h) = preserve_aspect_ratio.compute (vbox.0.width,
                                                                  vbox.0.height,
                                                                  0.0,
                                                                  0.0,
                                                                  pattern_width * bbwscale,
                                                                  pattern_height * bbhscale);

        x -= vbox.0.x * w / vbox.0.width;
        y -= vbox.0.y * h / vbox.0.height;

        caffine = cairo::Matrix::new (w / vbox.0.width,
                                      0.0,
                                      0.0,
                                      h / vbox.0.height,
                                      x,
                                      y);

        drawing_ctx::push_view_box (draw_ctx, vbox.0.width, vbox.0.height);
        pushed_view_box = true;
    } else if content_units == PatternContentUnits (PaintServerUnits::ObjectBoundingBox) {
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

    let surface = cr_save.get_target ().create_similar (cairo::Content::ColorAlpha, pw, ph);

    let cr_pattern = cairo::Context::new (&surface);

    drawing_ctx::set_cairo_context (draw_ctx, &cr_pattern);

    // Set up transformations to be determined by the contents units
    drawing_ctx::set_current_state_affine (draw_ctx, caffine);

    // Draw everything
    let pattern_node = pattern.node.clone ().unwrap ().upgrade ().unwrap ();
    pattern_node.draw_children (draw_ctx, 2);

    // Return to the original coordinate system and rendering context

    drawing_ctx::state_pop (draw_ctx);
    drawing_ctx::set_cairo_context (draw_ctx, &cr_save);

    if pushed_view_box {
        drawing_ctx::pop_view_box (draw_ctx);
    }

    // Set the final surface as a Cairo pattern into the Cairo context

    let surface_pattern = cairo::SurfacePattern::create (&surface);
    surface_pattern.set_extend (cairo::Extend::Repeat);

    let mut matrix = affine;
    matrix.invert ();

    surface_pattern.set_matrix (matrix);
    surface_pattern.set_filter (cairo::Filter::Best);

    cr_save.set_source (&surface_pattern);

    true
}

fn resolve_fallbacks_and_set_pattern (pattern:  &Pattern,
                                      draw_ctx: *mut RsvgDrawingCtx,
                                      bbox:     RsvgBbox) -> bool {
    let mut fallback_source = NodeFallbackSource::new (draw_ctx);

    let resolved = resolve_pattern (pattern, &mut fallback_source);

    set_pattern_on_draw_context (&resolved, draw_ctx, &bbox)
}

#[no_mangle]
pub extern fn rsvg_node_pattern_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::Pattern,
                    raw_parent,
                    Box::new (NodePattern::new ()))
}

#[no_mangle]
pub extern fn pattern_resolve_fallbacks_and_set_pattern (raw_node: *const RsvgNode,
                                                         draw_ctx: *mut RsvgDrawingCtx,
                                                         bbox:     RsvgBbox) -> glib_sys::gboolean {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    assert! (node.get_type () == NodeType::Pattern);

    let mut did_set_pattern = false;

    node.with_impl (|node_pattern: &NodePattern| {
        let pattern = &*node_pattern.pattern.borrow ();
        did_set_pattern = resolve_fallbacks_and_set_pattern (pattern, draw_ctx, bbox);
    });

    did_set_pattern.to_glib ()
}
