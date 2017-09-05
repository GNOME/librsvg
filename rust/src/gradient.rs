use ::cairo;
use ::glib_sys;
use ::glib::translate::*;
use ::libc;

use std::cell::RefCell;

use cairo::MatrixTrait;

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
use stop::*;
use util::*;


#[derive(Copy, Clone)]
pub struct ColorStop {
    pub offset: f64,
    pub rgba:   u32
}

/* Any of the attributes in gradient elements may be omitted.  In turn, the missing
 * ones can be inherited from the gradient referenced by its "fallback" IRI.  We
 * represent these possibly-missing attributes as Option<foo>.
 */
#[derive(Clone)]
pub struct GradientCommon {
    pub units:    Option<PaintServerUnits>,
    pub affine:   Option<cairo::Matrix>,
    pub spread:   Option<PaintServerSpread>,
    pub fallback: Option<String>,
    pub stops:    Option<Vec<ColorStop>>
}

#[derive(Copy, Clone)]
pub enum GradientVariant {
    Linear {
        x1: Option<RsvgLength>,
        y1: Option<RsvgLength>,
        x2: Option<RsvgLength>,
        y2: Option<RsvgLength>
    },

    Radial {
        cx: Option<RsvgLength>,
        cy: Option<RsvgLength>,
        r:  Option<RsvgLength>,
        fx: Option<RsvgLength>,
        fy: Option<RsvgLength>,
    }
}

#[derive(Clone)]
pub struct Gradient {
    pub common: GradientCommon,
    pub variant: GradientVariant
}

impl Default for GradientCommon {
    fn default () -> GradientCommon {
        GradientCommon {
            units:    None,
            affine:   None,
            spread:   None,
            fallback: None,
            stops:    None,
        }
    }
}

// All of the Gradient's fields are Option<foo> values, because
// those fields can be omitted in the SVG file.  We need to resolve
// them to default values, or to fallback values that come from
// another Gradient.
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

impl GradientCommon {
    fn clone_stops (&self) -> Option<Vec<ColorStop>> {
        if let Some (ref stops) = self.stops {
            Some (stops.clone ())
        } else {
            None
        }
    }

    fn is_resolved (&self) -> bool {
        self.units.is_some() &&
            self.affine.is_some () &&
            self.spread.is_some () &&
            self.stops.is_some ()
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        fallback_to! (self.units,  Some (PaintServerUnits::default ()));
        fallback_to! (self.affine, Some (cairo::Matrix::identity ()));
        fallback_to! (self.spread, Some (PaintServerSpread::default ()));
        fallback_to! (self.stops,  Some (Vec::<ColorStop>::new ())); // empty array of color stops

        self.fallback = None;
    }

    fn resolve_from_fallback (&mut self, fallback: &GradientCommon) {
        fallback_to! (self.units,  fallback.units);
        fallback_to! (self.affine, fallback.affine);
        fallback_to! (self.spread, fallback.spread);
        fallback_to! (self.stops,  fallback.clone_stops ());

        self.fallback = clone_fallback_name (&fallback.fallback);
    }

    fn add_color_stop (&mut self, mut offset: f64, rgba: u32) {
        if self.stops.is_none () {
            self.stops = Some (Vec::<ColorStop>::new ());
        }

        if let Some (ref mut stops) = self.stops {
            let mut last_offset: f64 = 0.0;

            if stops.len () > 0 {
                last_offset = stops[stops.len () - 1].offset;
            }

            if last_offset > offset {
                offset = last_offset;
            }

            stops.push (ColorStop { offset: offset,
                                    rgba:   rgba });
        } else {
            unreachable! ();
        }
    }
}

impl GradientVariant {
    fn is_resolved (&self) -> bool {
        match *self {
            GradientVariant::Linear { x1, y1, x2, y2 } => {
                x1.is_some () &&
                    y1.is_some () &&
                    x2.is_some () &&
                    y2.is_some ()
            },

            GradientVariant::Radial { cx, cy, r, fx, fy } => {
                cx.is_some () &&
                    cy.is_some () &&
                    r.is_some () &&
                    fx.is_some () &&
                    fy.is_some ()
            }
        }
    }

    fn resolve_from_defaults (&mut self) {
        /* These are per the spec */

        match *self {
            // https://www.w3.org/TR/SVG/pservers.html#LinearGradients
            GradientVariant::Linear { ref mut x1, ref mut y1, ref mut x2, ref mut y2 } => {
                fallback_to! (*x1, Some (RsvgLength::parse ("0%", LengthDir::Horizontal).unwrap ()));
                fallback_to! (*y1, Some (RsvgLength::parse ("0%", LengthDir::Vertical).unwrap ()));
                fallback_to! (*x2, Some (RsvgLength::parse ("100%", LengthDir::Horizontal).unwrap ()));
                fallback_to! (*y2, Some (RsvgLength::parse ("0%", LengthDir::Vertical).unwrap ()));
            },

            // https://www.w3.org/TR/SVG/pservers.html#RadialGradients
            GradientVariant::Radial { ref mut cx, ref mut cy, ref mut r, ref mut fx, ref mut fy } => {
                fallback_to! (*cx, Some (RsvgLength::parse ("50%", LengthDir::Horizontal).unwrap ()));
                fallback_to! (*cy, Some (RsvgLength::parse ("50%", LengthDir::Vertical).unwrap ()));
                fallback_to! (*r,  Some (RsvgLength::parse ("50%", LengthDir::Both).unwrap ()));

                /* fx and fy fall back to the presentational value of cx and cy */
                fallback_to! (*fx, *cx);
                fallback_to! (*fy, *cy);
            }
        }
    }

    fn resolve_from_fallback (&mut self, fallback: &GradientVariant) {
        match *self {
            GradientVariant::Linear { ref mut x1, ref mut y1, ref mut x2, ref mut y2 } => {
                if let &GradientVariant::Linear { x1: x1f, y1: y1f, x2: x2f, y2: y2f } = fallback {
                    fallback_to! (*x1, x1f);
                    fallback_to! (*y1, y1f);
                    fallback_to! (*x2, x2f);
                    fallback_to! (*y2, y2f);
                }
            },

            GradientVariant::Radial { ref mut cx, ref mut cy, ref mut r, ref mut fx, ref mut fy } => {
                if let &GradientVariant::Radial { cx: cxf, cy: cyf, r: rf, fx: fxf, fy: fyf } = fallback {
                    fallback_to! (*cx, cxf);
                    fallback_to! (*cy, cyf);
                    fallback_to! (*r,  rf);
                    fallback_to! (*fx, fxf);
                    fallback_to! (*fy, fyf);
                }
            }
        }
    }
}

impl Gradient {
    fn is_resolved (&self) -> bool {
        self.common.is_resolved () && self.variant.is_resolved ()
    }

    fn resolve_from_defaults (&mut self) {
        self.common.resolve_from_defaults ();
        self.variant.resolve_from_defaults ();
    }

    fn resolve_from_fallback (&mut self, fallback: &Gradient) {
        self.common.resolve_from_fallback (&fallback.common);
        self.variant.resolve_from_fallback (&fallback.variant);
    }

    fn add_color_stops_from_node (&mut self, node: &RsvgNode) {
        assert! (node.get_type () == NodeType::LinearGradient || node.get_type () == NodeType::RadialGradient);

        for child in &*node.children.borrow () {
            if child.get_type () != NodeType::Stop {
                continue; // just ignore this child; we are only interested in gradient stops
            }

            if child.get_result ().is_err () {
                break; // don't add any more stops
            }

            child.with_impl (|stop: &NodeStop| {
                self.add_color_stop (stop.get_offset (), stop.get_rgba ());
            });
        }
    }

    fn add_color_stop (&mut self, offset: f64, rgba: u32) {
        self.common.add_color_stop (offset, rgba);
    }

    fn add_color_stops_to_pattern (&self,
                                   pattern:  &mut cairo::Gradient,
                                   opacity:  u8) {
        let stops = self.common.stops.as_ref ().unwrap ();

        for stop in stops {
            let rgba = stop.rgba;
            pattern.add_color_stop_rgba (stop.offset,
                                         ((rgba >> 24) & 0xff) as f64 / 255.0,
                                         ((rgba >> 16) & 0xff) as f64 / 255.0,
                                         ((rgba >> 8) & 0xff) as f64 / 255.0,
                                         (((rgba >> 0) & 0xff) * opacity as u32) as f64 / 255.0 / 255.0);
        }
    }
}

trait FallbackSource {
    fn get_fallback (&mut self, name: &str) -> Option<RsvgNode>;
}

fn resolve_gradient (gradient: &Gradient, fallback_source: &mut FallbackSource) -> Gradient {
    let mut result = gradient.clone ();

    while !result.is_resolved () {
        let mut opt_fallback: Option<RsvgNode> = None;

        if let Some (ref fallback_name) = result.common.fallback {
            opt_fallback = fallback_source.get_fallback (&**fallback_name);
        }

        if let Some (fallback_node) = opt_fallback {
            fallback_node.with_impl (|i: &NodeGradient| {
                let fallback_gradient = i.get_gradient_with_color_stops_from_node (&fallback_node);
                result.resolve_from_fallback (&fallback_gradient)
            });
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
        let fallback_node = drawing_ctx::acquire_node (self.draw_ctx, name);

        if fallback_node.is_null () {
            return None;
        }

        let node: &RsvgNode = unsafe { & *fallback_node };
        if !(node.get_type () == NodeType::LinearGradient || node.get_type () == NodeType::RadialGradient) {
            return None;
        }

        self.acquired_nodes.push (fallback_node);

        return Some (node.clone ());
    }
}

fn set_common_on_pattern<P: cairo::Pattern + cairo::Gradient> (gradient: &Gradient,
                                                               draw_ctx: *mut RsvgDrawingCtx,
                                                               pattern:  &mut P,
                                                               bbox:     &RsvgBbox,
                                                               opacity:  u8)
{
    let cr = drawing_ctx::get_cairo_context (draw_ctx);

    let mut affine = gradient.common.affine.unwrap ();

    let units = gradient.common.units.unwrap ();

    if units == PaintServerUnits::ObjectBoundingBox {
        let bbox_matrix = cairo::Matrix::new (bbox.rect.width, 0.0,
                                              0.0, bbox.rect.height,
                                              bbox.rect.x, bbox.rect.y);
        affine = cairo::Matrix::multiply (&affine, &bbox_matrix);
    }

    affine.invert ();
    pattern.set_matrix (affine);
    pattern.set_extend (gradient.common.spread.unwrap ().0);

    gradient.add_color_stops_to_pattern (pattern, opacity);

    cr.set_source (pattern);
}

fn set_linear_gradient_on_pattern (gradient: &Gradient,
                                   draw_ctx: *mut RsvgDrawingCtx,
                                   bbox:     &RsvgBbox,
                                   opacity:  u8) -> bool {
    if let GradientVariant::Linear { x1, y1, x2, y2 } = gradient.variant {
        let units = gradient.common.units.unwrap ();

        if units == PaintServerUnits::ObjectBoundingBox {
            drawing_ctx::push_view_box (draw_ctx, 1.0, 1.0);
        }

        let mut pattern = cairo::LinearGradient::new (x1.as_ref ().unwrap ().normalize (draw_ctx),
                                                      y1.as_ref ().unwrap ().normalize (draw_ctx),
                                                      x2.as_ref ().unwrap ().normalize (draw_ctx),
                                                      y2.as_ref ().unwrap ().normalize (draw_ctx));

        if units == PaintServerUnits::ObjectBoundingBox {
            drawing_ctx::pop_view_box (draw_ctx);
        }

        set_common_on_pattern (gradient, draw_ctx, &mut pattern, bbox, opacity);
    } else {
        unreachable! ();
    }

    true
}

/* SVG defines radial gradients as being inside a circle (cx, cy, radius).  The
 * gradient projects out from a focus point (fx, fy), which is assumed to be
 * inside the circle, to the edge of the circle.
 *
 * The description of https://www.w3.org/TR/SVG/pservers.html#RadialGradientElement
 * states:
 *
 * If the point defined by ‘fx’ and ‘fy’ lies outside the circle defined by
 * ‘cx’, ‘cy’ and ‘r’, then the user agent shall set the focal point to the
 * intersection of the line from (‘cx’, ‘cy’) to (‘fx’, ‘fy’) with the circle
 * defined by ‘cx’, ‘cy’ and ‘r’.
 *
 * So, let's do that!
 */
fn fix_focus_point (mut fx: f64,
                    mut fy: f64,
                    cx: f64,
                    cy: f64,
                    radius: f64) -> (f64, f64) {
    /* Easy case first: the focus point is inside the circle */

    if (fx - cx) * (fx - cx) + (fy - cy) * (fy - cy) <= radius * radius {
        return (fx, fy);
    }

    /* Hard case: focus point is outside the circle.
     *
     * First, translate everything to the origin.
     */

    fx -= cx;
    fy -= cy;

    /* Find the vector from the origin to (fx, fy) */

    let mut vx = fx;
    let mut vy = fy;

    /* Find the vector's magnitude */

    let mag = (vx * vx + vy * vy).sqrt ();

    /* Normalize the vector to have a magnitude equal to radius; (vx, vy) will now be on the edge of the circle */

    let scale = mag / radius;

    vx /= scale;
    vy /= scale;

    /* Translate back to (cx, cy) and we are done! */

    (vx + cx, vy + cy)
}

fn set_radial_gradient_on_pattern (gradient: &Gradient,
                                   draw_ctx: *mut RsvgDrawingCtx,
                                   bbox:     &RsvgBbox,
                                   opacity:  u8) -> bool {
    if let GradientVariant::Radial { cx, cy, r, fx, fy } = gradient.variant {
        let units = gradient.common.units.unwrap ();

        if units == PaintServerUnits::ObjectBoundingBox {
            drawing_ctx::push_view_box (draw_ctx, 1.0, 1.0);
        }

        let n_cx = cx.as_ref ().unwrap ().normalize (draw_ctx);
        let n_cy = cy.as_ref ().unwrap ().normalize (draw_ctx);
        let n_r  =  r.as_ref ().unwrap ().normalize (draw_ctx);
        let n_fx = fx.as_ref ().unwrap ().normalize (draw_ctx);
        let n_fy = fy.as_ref ().unwrap ().normalize (draw_ctx);

        let (new_fx, new_fy) = fix_focus_point (n_fx, n_fy, n_cx, n_cy, n_r);

        let mut pattern = cairo::RadialGradient::new (new_fx, new_fy, 0.0, n_cx, n_cy, n_r);

        if units == PaintServerUnits::ObjectBoundingBox {
            drawing_ctx::pop_view_box (draw_ctx);
        }

        set_common_on_pattern (gradient, draw_ctx, &mut pattern, bbox, opacity);
    } else {
        unreachable! ();
    }

    true
}

fn set_pattern_on_draw_context (gradient: &Gradient,
                                draw_ctx: *mut RsvgDrawingCtx,
                                opacity:  u8,
                                bbox:     &RsvgBbox) -> bool {
    assert! (gradient.is_resolved ());

    match gradient.variant {
        GradientVariant::Linear { .. } => {
            set_linear_gradient_on_pattern (gradient, draw_ctx, bbox, opacity)
        }

        GradientVariant::Radial { .. } => {
            set_radial_gradient_on_pattern (gradient, draw_ctx, bbox, opacity)
        }
    }
}

struct NodeGradient {
    gradient: RefCell <Gradient>
}

impl NodeGradient {
    fn new_linear () -> NodeGradient {
        NodeGradient {
            gradient: RefCell::new (Gradient {
                common: GradientCommon::default (),
                variant: GradientVariant::Linear {
                    x1: None,
                    y1: None,
                    x2: None,
                    y2: None
                }
            })
        }
    }

    fn new_radial () -> NodeGradient {
        NodeGradient {
            gradient: RefCell::new (Gradient {
                common: GradientCommon::default (),
                variant: GradientVariant::Radial {
                    cx: None,
                    cy: None,
                    r:  None,
                    fx: None,
                    fy: None
                }
            })
        }
    }

    fn get_gradient_with_color_stops_from_node (&self, node: &RsvgNode) -> Gradient {
        let mut gradient = self.gradient.borrow ().clone ();
        gradient.add_color_stops_from_node (node);
        gradient
    }
}

impl NodeTrait for NodeGradient {
    fn set_atts (&self, node: &RsvgNode, _: *const RsvgHandle, pbag: *const RsvgPropertyBag) -> NodeResult {
        let mut g = self.gradient.borrow_mut ();

        // Attributes common to linear and radial gradients

        g.common.units    = property_bag::parse_or_none (pbag, "gradientUnits", (), None)?;
        g.common.affine   = property_bag::parse_or_none (pbag, "gradientTransform", (), None)?;
        g.common.spread   = property_bag::parse_or_none (pbag, "spreadMethod", (), None)?;
        g.common.fallback = property_bag::lookup (pbag, "xlink:href");

        // Attributes specific to each gradient type.  The defaults mandated by the spec
        // are in GradientVariant::resolve_from_defaults()

        match node.get_type () {
            NodeType::LinearGradient => {
                g.variant = GradientVariant::Linear {
                    x1: property_bag::parse_or_none (pbag, "x1", LengthDir::Horizontal, None)?,
                    y1: property_bag::parse_or_none (pbag, "y1", LengthDir::Vertical, None)?,
                    x2: property_bag::parse_or_none (pbag, "x2", LengthDir::Horizontal, None)?,
                    y2: property_bag::parse_or_none (pbag, "y2", LengthDir::Vertical, None)?
                };
            },

            NodeType::RadialGradient => {
                g.variant = GradientVariant::Radial {
                    cx: property_bag::parse_or_none (pbag, "cx", LengthDir::Horizontal, None)?,
                    cy: property_bag::parse_or_none (pbag, "cy", LengthDir::Vertical, None)?,
                    r:  property_bag::parse_or_none (pbag, "r",  LengthDir::Both, None)?,
                    fx: property_bag::parse_or_none (pbag, "fx", LengthDir::Horizontal, None)?,
                    fy: property_bag::parse_or_none (pbag, "fy", LengthDir::Vertical, None)?
                };
            },

            _ => unreachable! ()
        }

        Ok (())
    }

    fn draw (&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing; paint servers are handled specially
    }

    fn get_c_impl (&self) -> *const RsvgCNodeImpl {
        unreachable! ();
    }
}

#[no_mangle]
pub extern fn rsvg_node_linear_gradient_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::LinearGradient,
                    raw_parent,
                    Box::new (NodeGradient::new_linear ()))
}

#[no_mangle]
pub extern fn rsvg_node_radial_gradient_new (_: *const libc::c_char, raw_parent: *const RsvgNode) -> *const RsvgNode {
    boxed_node_new (NodeType::RadialGradient,
                    raw_parent,
                    Box::new (NodeGradient::new_radial ()))
}

fn resolve_fallbacks_and_set_pattern (gradient: &Gradient,
                                      draw_ctx: *mut RsvgDrawingCtx,
                                      opacity:  u8,
                                      bbox:     RsvgBbox) -> bool {
    let mut fallback_source = NodeFallbackSource::new (draw_ctx);

    let resolved = resolve_gradient (gradient, &mut fallback_source);

    set_pattern_on_draw_context (&resolved, draw_ctx, opacity, &bbox)
}

#[no_mangle]
pub extern fn gradient_resolve_fallbacks_and_set_pattern (raw_node:     *const RsvgNode,
                                                          draw_ctx:     *mut RsvgDrawingCtx,
                                                          opacity:      u8,
                                                          bbox:         RsvgBbox) -> glib_sys::gboolean {
    assert! (!raw_node.is_null ());
    let node: &RsvgNode = unsafe { & *raw_node };

    assert! (node.get_type () == NodeType::LinearGradient || node.get_type () == NodeType::RadialGradient);

    let mut did_set_gradient = false;

    node.with_impl (|node_gradient: &NodeGradient| {
        let gradient = node_gradient.get_gradient_with_color_stops_from_node (&node);
        did_set_gradient = resolve_fallbacks_and_set_pattern (&gradient, draw_ctx, opacity, bbox);
    });

    did_set_gradient.to_glib ()
}
