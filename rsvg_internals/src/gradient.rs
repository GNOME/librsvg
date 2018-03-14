use cairo;
use libc;

use std::cell::RefCell;

use cairo::MatrixTrait;

use attributes::Attribute;
use bbox::*;
use coord_units::CoordUnits;
use drawing_ctx::{self, AcquiredNode, RsvgDrawingCtx};
use handle::RsvgHandle;
use length::*;
use node::*;
use paint_server::*;
use parsers::{parse, Parse};
use property_bag::PropertyBag;
use stop::*;
use util::*;

#[derive(Copy, Clone)]
struct ColorStop {
    pub offset: f64,
    pub rgba: u32,
}

coord_units!(GradientUnits, CoordUnits::ObjectBoundingBox);

// Any of the attributes in gradient elements may be omitted.  In turn, the missing
// ones can be inherited from the gradient referenced by its "fallback" IRI.  We
// represent these possibly-missing attributes as Option<foo>.
#[derive(Clone)]
struct GradientCommon {
    pub units: Option<GradientUnits>,
    pub affine: Option<cairo::Matrix>,
    pub spread: Option<PaintServerSpread>,
    pub fallback: Option<String>,
    pub stops: Option<Vec<ColorStop>>,
}

#[derive(Copy, Clone)]
enum GradientVariant {
    Linear {
        x1: Option<RsvgLength>,
        y1: Option<RsvgLength>,
        x2: Option<RsvgLength>,
        y2: Option<RsvgLength>,
    },

    Radial {
        cx: Option<RsvgLength>,
        cy: Option<RsvgLength>,
        r: Option<RsvgLength>,
        fx: Option<RsvgLength>,
        fy: Option<RsvgLength>,
    },
}

#[derive(Clone)]
struct Gradient {
    pub common: GradientCommon,
    pub variant: GradientVariant,
}

impl Default for GradientCommon {
    fn default() -> GradientCommon {
        GradientCommon {
            units: Some(GradientUnits::default()),
            affine: Some(cairo::Matrix::identity()),
            spread: Some(PaintServerSpread::default()),
            fallback: None,
            stops: Some(Vec::<ColorStop>::new()),
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
    fn unresolved() -> GradientCommon {
        GradientCommon {
            units: None,
            affine: None,
            spread: None,
            fallback: None,
            stops: None,
        }
    }

    fn clone_stops(&self) -> Option<Vec<ColorStop>> {
        if let Some(ref stops) = self.stops {
            Some(stops.clone())
        } else {
            None
        }
    }

    fn is_resolved(&self) -> bool {
        self.units.is_some() && self.affine.is_some() && self.spread.is_some()
            && self.stops.is_some()
    }

    fn resolve_from_defaults(&mut self) {
        self.resolve_from_fallback(&GradientCommon::default());
    }

    fn resolve_from_fallback(&mut self, fallback: &GradientCommon) {
        fallback_to!(self.units, fallback.units);
        fallback_to!(self.affine, fallback.affine);
        fallback_to!(self.spread, fallback.spread);
        fallback_to!(self.stops, fallback.clone_stops());

        self.fallback = clone_fallback_name(&fallback.fallback);
    }

    fn add_color_stop(&mut self, mut offset: f64, rgba: u32) {
        if self.stops.is_none() {
            self.stops = Some(Vec::<ColorStop>::new());
        }

        if let Some(ref mut stops) = self.stops {
            let last_offset: f64 = if !stops.is_empty() {
                stops[stops.len() - 1].offset
            } else {
                0.0
            };

            if last_offset > offset {
                offset = last_offset;
            }

            stops.push(ColorStop { offset, rgba });
        } else {
            unreachable!();
        }
    }
}

impl GradientVariant {
    fn unresolved_linear() -> Self {
        GradientVariant::Linear {
            x1: None,
            y1: None,
            x2: None,
            y2: None,
        }
    }

    fn unresolved_radial() -> Self {
        GradientVariant::Radial {
            cx: None,
            cy: None,
            r: None,
            fx: None,
            fy: None,
        }
    }

    fn is_resolved(&self) -> bool {
        match *self {
            GradientVariant::Linear { x1, y1, x2, y2 } => {
                x1.is_some() && y1.is_some() && x2.is_some() && y2.is_some()
            }

            GradientVariant::Radial { cx, cy, r, fx, fy } => {
                cx.is_some() && cy.is_some() && r.is_some() && fx.is_some() && fy.is_some()
            }
        }
    }

    fn default_linear() -> Self {
        // https://www.w3.org/TR/SVG/pservers.html#LinearGradients

        GradientVariant::Linear {
            x1: Some(RsvgLength::parse("0%", LengthDir::Horizontal).unwrap()),
            y1: Some(RsvgLength::parse("0%", LengthDir::Vertical).unwrap()),
            x2: Some(RsvgLength::parse("100%", LengthDir::Horizontal).unwrap()),
            y2: Some(RsvgLength::parse("0%", LengthDir::Vertical).unwrap()),
        }
    }

    fn default_radial() -> Self {
        // https://www.w3.org/TR/SVG/pservers.html#RadialGradients

        GradientVariant::Radial {
            cx: Some(RsvgLength::parse("50%", LengthDir::Horizontal).unwrap()),
            cy: Some(RsvgLength::parse("50%", LengthDir::Vertical).unwrap()),
            r: Some(RsvgLength::parse("50%", LengthDir::Both).unwrap()),

            fx: None,
            fy: None,
        }
    }

    fn resolve_from_defaults(&mut self) {
        // These are per the spec
        match *self {
            GradientVariant::Linear { .. } => {
                self.resolve_from_fallback(&GradientVariant::default_linear())
            }

            GradientVariant::Radial { .. } => {
                self.resolve_from_fallback(&GradientVariant::default_radial());
            }
        }

        if let GradientVariant::Radial {
            cx,
            cy,
            ref mut fx,
            ref mut fy,
            ..
        } = *self
        {
            // fx and fy fall back to the presentational value of cx and cy
            fallback_to!(*fx, cx);
            fallback_to!(*fy, cy);
        }
    }

    fn resolve_from_fallback(&mut self, fallback: &GradientVariant) {
        match *self {
            GradientVariant::Linear {
                ref mut x1,
                ref mut y1,
                ref mut x2,
                ref mut y2,
            } => {
                if let GradientVariant::Linear {
                    x1: x1f,
                    y1: y1f,
                    x2: x2f,
                    y2: y2f,
                } = *fallback
                {
                    fallback_to!(*x1, x1f);
                    fallback_to!(*y1, y1f);
                    fallback_to!(*x2, x2f);
                    fallback_to!(*y2, y2f);
                }
            }

            GradientVariant::Radial {
                ref mut cx,
                ref mut cy,
                ref mut r,
                ref mut fx,
                ref mut fy,
            } => {
                if let GradientVariant::Radial {
                    cx: cxf,
                    cy: cyf,
                    r: rf,
                    fx: fxf,
                    fy: fyf,
                } = *fallback
                {
                    fallback_to!(*cx, cxf);
                    fallback_to!(*cy, cyf);
                    fallback_to!(*r, rf);
                    fallback_to!(*fx, fxf);
                    fallback_to!(*fy, fyf);
                }
            }
        }
    }
}

impl Gradient {
    fn is_resolved(&self) -> bool {
        self.common.is_resolved() && self.variant.is_resolved()
    }

    fn resolve_from_defaults(&mut self) {
        self.common.resolve_from_defaults();
        self.variant.resolve_from_defaults();
    }

    fn resolve_from_fallback(&mut self, fallback: &Gradient) {
        self.common.resolve_from_fallback(&fallback.common);
        self.variant.resolve_from_fallback(&fallback.variant);
    }

    fn add_color_stops_from_node(&mut self, node: &RsvgNode) {
        assert!(
            node.get_type() == NodeType::LinearGradient
                || node.get_type() == NodeType::RadialGradient
        );

        node.foreach_child(|child| {
            if child.get_type() != NodeType::Stop {
                return true; // just ignore this child; we are only interested in gradient stops
            }

            if child.get_result().is_err() {
                return false; // don't add any more stops
            }

            child.with_impl(|stop: &NodeStop| {
                self.add_color_stop(stop.get_offset(), stop.get_rgba());
            });

            true
        });
    }

    fn add_color_stop(&mut self, offset: f64, rgba: u32) {
        self.common.add_color_stop(offset, rgba);
    }

    fn add_color_stops_to_pattern(&self, pattern: &mut cairo::Gradient, opacity: u8) {
        if let Some(stops) = self.common.stops.as_ref() {
            for stop in stops {
                let rgba = stop.rgba;
                pattern.add_color_stop_rgba(
                    stop.offset,
                    (f64::from((rgba >> 24) & 0xff)) / 255.0,
                    (f64::from((rgba >> 16) & 0xff)) / 255.0,
                    (f64::from((rgba >> 8) & 0xff)) / 255.0,
                    f64::from((rgba & 0xff) * u32::from(opacity)) / 255.0 / 255.0,
                );
            }
        }
    }
}

fn acquire_gradient(draw_ctx: *mut RsvgDrawingCtx, name: &str) -> Option<AcquiredNode> {
    drawing_ctx::get_acquired_node(draw_ctx, name).and_then(|acquired| {
        // FIXME: replace with .filter() once Option.filter() becomes stable
        let node = acquired.get();
        if node.get_type() == NodeType::LinearGradient
            || node.get_type() == NodeType::RadialGradient
        {
            Some(acquired)
        } else {
            None
        }
    })
}

fn resolve_gradient(gradient: &Gradient, draw_ctx: *mut RsvgDrawingCtx) -> Gradient {
    let mut result = gradient.clone();

    while !result.is_resolved() {
        result
            .common
            .fallback
            .as_ref()
            .and_then(|fallback_name| acquire_gradient(draw_ctx, fallback_name))
            .and_then(|acquired| {
                let fallback_node = acquired.get();

                fallback_node.with_impl(|i: &NodeGradient| {
                    let fallback_grad = i.get_gradient_with_color_stops_from_node(&fallback_node);
                    result.resolve_from_fallback(&fallback_grad)
                });
                Some(())
            })
            .or_else(|| {
                result.resolve_from_defaults();
                Some(())
            });
    }

    result
}

fn set_common_on_pattern<P: cairo::Pattern + cairo::Gradient>(
    gradient: &Gradient,
    draw_ctx: *mut RsvgDrawingCtx,
    pattern: &mut P,
    bbox: &RsvgBbox,
    opacity: u8,
) {
    let cr = drawing_ctx::get_cairo_context(draw_ctx);

    let mut affine = gradient.common.affine.unwrap();

    let units = gradient.common.units.unwrap();

    if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
        let bbox_matrix = cairo::Matrix::new(
            bbox.rect.width,
            0.0,
            0.0,
            bbox.rect.height,
            bbox.rect.x,
            bbox.rect.y,
        );
        affine = cairo::Matrix::multiply(&affine, &bbox_matrix);
    }

    affine.invert();
    pattern.set_matrix(affine);
    pattern.set_extend(gradient.common.spread.unwrap().0);

    gradient.add_color_stops_to_pattern(pattern, opacity);

    cr.set_source(pattern);
}

fn set_linear_gradient_on_pattern(
    gradient: &Gradient,
    draw_ctx: *mut RsvgDrawingCtx,
    bbox: &RsvgBbox,
    opacity: u8,
) -> bool {
    if let GradientVariant::Linear { x1, y1, x2, y2 } = gradient.variant {
        let units = gradient.common.units.unwrap();

        if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            drawing_ctx::push_view_box(draw_ctx, 1.0, 1.0);
        }

        let mut pattern = cairo::LinearGradient::new(
            x1.as_ref().unwrap().normalize(draw_ctx),
            y1.as_ref().unwrap().normalize(draw_ctx),
            x2.as_ref().unwrap().normalize(draw_ctx),
            y2.as_ref().unwrap().normalize(draw_ctx),
        );

        if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            drawing_ctx::pop_view_box(draw_ctx);
        }

        set_common_on_pattern(gradient, draw_ctx, &mut pattern, bbox, opacity);
    } else {
        unreachable!();
    }

    true
}

// SVG defines radial gradients as being inside a circle (cx, cy, radius).  The
// gradient projects out from a focus point (fx, fy), which is assumed to be
// inside the circle, to the edge of the circle.
// The description of https://www.w3.org/TR/SVG/pservers.html#RadialGradientElement
// states:
//
// If the point defined by ‘fx’ and ‘fy’ lies outside the circle defined by
// ‘cx’, ‘cy’ and ‘r’, then the user agent shall set the focal point to the
// intersection of the line from (‘cx’, ‘cy’) to (‘fx’, ‘fy’) with the circle
// defined by ‘cx’, ‘cy’ and ‘r’.
//
// So, let's do that!
fn fix_focus_point(mut fx: f64, mut fy: f64, cx: f64, cy: f64, radius: f64) -> (f64, f64) {
    // Easy case first: the focus point is inside the circle

    if (fx - cx) * (fx - cx) + (fy - cy) * (fy - cy) <= radius * radius {
        return (fx, fy);
    }

    // Hard case: focus point is outside the circle.
    // First, translate everything to the origin.

    fx -= cx;
    fy -= cy;

    // Find the vector from the origin to (fx, fy)

    let mut vx = fx;
    let mut vy = fy;

    // Find the vector's magnitude

    let mag = (vx * vx + vy * vy).sqrt();

    // Normalize the vector to have a magnitude equal to radius; (vx, vy) will now be on the
    // edge of the circle

    let scale = mag / radius;

    vx /= scale;
    vy /= scale;

    // Translate back to (cx, cy) and we are done!

    (vx + cx, vy + cy)
}

fn set_radial_gradient_on_pattern(
    gradient: &Gradient,
    draw_ctx: *mut RsvgDrawingCtx,
    bbox: &RsvgBbox,
    opacity: u8,
) -> bool {
    if let GradientVariant::Radial { cx, cy, r, fx, fy } = gradient.variant {
        let units = gradient.common.units.unwrap();

        if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            drawing_ctx::push_view_box(draw_ctx, 1.0, 1.0);
        }

        let n_cx = cx.as_ref().unwrap().normalize(draw_ctx);
        let n_cy = cy.as_ref().unwrap().normalize(draw_ctx);
        let n_r = r.as_ref().unwrap().normalize(draw_ctx);
        let n_fx = fx.as_ref().unwrap().normalize(draw_ctx);
        let n_fy = fy.as_ref().unwrap().normalize(draw_ctx);

        let (new_fx, new_fy) = fix_focus_point(n_fx, n_fy, n_cx, n_cy, n_r);

        let mut pattern = cairo::RadialGradient::new(new_fx, new_fy, 0.0, n_cx, n_cy, n_r);

        if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            drawing_ctx::pop_view_box(draw_ctx);
        }

        set_common_on_pattern(gradient, draw_ctx, &mut pattern, bbox, opacity);
    } else {
        unreachable!();
    }

    true
}

fn set_pattern_on_draw_context(
    gradient: &Gradient,
    draw_ctx: *mut RsvgDrawingCtx,
    opacity: u8,
    bbox: &RsvgBbox,
) -> bool {
    assert!(gradient.is_resolved());

    match gradient.variant {
        GradientVariant::Linear { .. } => {
            set_linear_gradient_on_pattern(gradient, draw_ctx, bbox, opacity)
        }

        GradientVariant::Radial { .. } => {
            set_radial_gradient_on_pattern(gradient, draw_ctx, bbox, opacity)
        }
    }
}

struct NodeGradient {
    gradient: RefCell<Gradient>,
}

impl NodeGradient {
    fn new_linear() -> NodeGradient {
        NodeGradient {
            gradient: RefCell::new(Gradient {
                common: GradientCommon::unresolved(),
                variant: GradientVariant::unresolved_linear(),
            }),
        }
    }

    fn new_radial() -> NodeGradient {
        NodeGradient {
            gradient: RefCell::new(Gradient {
                common: GradientCommon::unresolved(),
                variant: GradientVariant::unresolved_radial(),
            }),
        }
    }

    fn get_gradient_with_color_stops_from_node(&self, node: &RsvgNode) -> Gradient {
        let mut gradient = self.gradient.borrow().clone();
        gradient.add_color_stops_from_node(node);
        gradient
    }
}

impl NodeTrait for NodeGradient {
    fn set_atts(&self, node: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag) -> NodeResult {
        let mut g = self.gradient.borrow_mut();

        let mut x1 = None;
        let mut y1 = None;
        let mut x2 = None;
        let mut y2 = None;

        let mut cx = None;
        let mut cy = None;
        let mut r = None;
        let mut fx = None;
        let mut fy = None;

        for (_key, attr, value) in pbag.iter() {
            match attr {
                // Attributes common to linear and radial gradients
                Attribute::GradientUnits => {
                    g.common.units = Some(parse("gradientUnits", value, (), None)?)
                }

                Attribute::GradientTransform => {
                    g.common.affine = Some(parse("gradientTransform", value, (), None)?)
                }

                Attribute::SpreadMethod => {
                    g.common.spread = Some(parse("spreadMethod", value, (), None)?)
                }

                Attribute::XlinkHref => g.common.fallback = Some(value.to_owned()),

                // Attributes specific to each gradient type.  The defaults mandated by the spec
                // are in GradientVariant::resolve_from_defaults()
                Attribute::X1 => x1 = Some(parse("x1", value, LengthDir::Horizontal, None)?),
                Attribute::Y1 => y1 = Some(parse("y1", value, LengthDir::Vertical, None)?),
                Attribute::X2 => x2 = Some(parse("x2", value, LengthDir::Horizontal, None)?),
                Attribute::Y2 => y2 = Some(parse("y2", value, LengthDir::Vertical, None)?),

                Attribute::Cx => cx = Some(parse("cx", value, LengthDir::Horizontal, None)?),
                Attribute::Cy => cy = Some(parse("cy", value, LengthDir::Vertical, None)?),
                Attribute::R => r = Some(parse("r", value, LengthDir::Both, None)?),
                Attribute::Fx => fx = Some(parse("fx", value, LengthDir::Horizontal, None)?),
                Attribute::Fy => fy = Some(parse("fy", value, LengthDir::Vertical, None)?),

                _ => (),
            }
        }

        match node.get_type() {
            NodeType::LinearGradient => {
                g.variant = GradientVariant::Linear { x1, y1, x2, y2 };
            }

            NodeType::RadialGradient => {
                g.variant = GradientVariant::Radial { cx, cy, r, fx, fy };
            }

            _ => unreachable!(),
        }

        Ok(())
    }

    fn draw(&self, _: &RsvgNode, _: *const RsvgDrawingCtx, _: i32) {
        // nothing; paint servers are handled specially
    }

    fn get_c_impl(&self) -> *const RsvgCNodeImpl {
        unreachable!();
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_linear_gradient_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::LinearGradient,
        raw_parent,
        Box::new(NodeGradient::new_linear()),
    )
}

#[no_mangle]
pub extern "C" fn rsvg_node_radial_gradient_new(
    _: *const libc::c_char,
    raw_parent: *const RsvgNode,
) -> *const RsvgNode {
    boxed_node_new(
        NodeType::RadialGradient,
        raw_parent,
        Box::new(NodeGradient::new_radial()),
    )
}

fn resolve_fallbacks_and_set_pattern(
    gradient: &Gradient,
    draw_ctx: *mut RsvgDrawingCtx,
    opacity: u8,
    bbox: &RsvgBbox,
) -> bool {
    if bbox.is_empty() {
        return true;
    }

    let resolved = resolve_gradient(gradient, draw_ctx);

    set_pattern_on_draw_context(&resolved, draw_ctx, opacity, bbox)
}

pub fn gradient_resolve_fallbacks_and_set_pattern(
    node: &RsvgNode,
    draw_ctx: *mut RsvgDrawingCtx,
    opacity: u8,
    bbox: &RsvgBbox,
) -> bool {
    assert!(
        node.get_type() == NodeType::LinearGradient || node.get_type() == NodeType::RadialGradient
    );

    let mut did_set_gradient = false;

    node.with_impl(|node_gradient: &NodeGradient| {
        let gradient = node_gradient.get_gradient_with_color_stops_from_node(node);
        did_set_gradient = resolve_fallbacks_and_set_pattern(&gradient, draw_ctx, opacity, bbox);
    });

    did_set_gradient
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gradient_resolved_from_defaults_is_really_resolved() {
        let mut gradient = Gradient {
            common: GradientCommon::unresolved(),
            variant: GradientVariant::unresolved_linear(),
        };

        gradient.resolve_from_defaults();
        assert!(gradient.is_resolved());
    }
}
