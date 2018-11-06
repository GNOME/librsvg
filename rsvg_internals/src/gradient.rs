use cairo::{self, MatrixTrait};
use cssparser::{self, CowRcStr, Parser, Token};

use std::cell::RefCell;

use attributes::Attribute;
use bbox::*;
use coord_units::CoordUnits;
use drawing_ctx::{AcquiredNode, DrawingCtx};
use error::*;
use handle::RsvgHandle;
use length::*;
use node::*;
use parsers::{parse, Parse, ParseError};
use property_bag::PropertyBag;
use rect::RectangleExt;
use state::{ComputedValues, StopColor};
use stop::*;
use unitinterval::UnitInterval;
use util::clone_fallback_name;

#[derive(Copy, Clone)]
struct ColorStop {
    pub offset: f64,
    pub rgba: cssparser::RGBA,
    pub opacity: UnitInterval,
}

coord_units!(GradientUnits, CoordUnits::ObjectBoundingBox);

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum SpreadMethod {
    Pad,
    Reflect,
    Repeat,
}

impl Parse for SpreadMethod {
    type Data = ();
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>, _: ()) -> Result<SpreadMethod, ValueErrorKind> {
        let loc = parser.current_source_location();

        parser
            .expect_ident()
            .and_then(|cow| match cow.as_ref() {
                "pad" => Ok(SpreadMethod::Pad),
                "reflect" => Ok(SpreadMethod::Reflect),
                "repeat" => Ok(SpreadMethod::Repeat),
                _ => Err(
                    loc.new_basic_unexpected_token_error(Token::Ident(CowRcStr::from(
                        cow.as_ref().to_string(),
                    ))),
                ),
            })
            .map_err(|_| {
                ValueErrorKind::Parse(ParseError::new("expected 'pad' | 'reflect' | 'repeat'"))
            })
    }
}

impl Default for SpreadMethod {
    fn default() -> SpreadMethod {
        SpreadMethod::Pad
    }
}

impl From<SpreadMethod> for cairo::enums::Extend {
    fn from(s: SpreadMethod) -> cairo::enums::Extend {
        match s {
            SpreadMethod::Pad => cairo::enums::Extend::Pad,
            SpreadMethod::Reflect => cairo::enums::Extend::Reflect,
            SpreadMethod::Repeat => cairo::enums::Extend::Repeat,
        }
    }
}

// Any of the attributes in gradient elements may be omitted.  In turn, the missing
// ones can be inherited from the gradient referenced by its "fallback" IRI.  We
// represent these possibly-missing attributes as Option<foo>.
#[derive(Clone)]
struct GradientCommon {
    pub units: Option<GradientUnits>,
    pub affine: Option<cairo::Matrix>,
    pub spread: Option<SpreadMethod>,
    pub fallback: Option<String>,
    pub stops: Option<Vec<ColorStop>>,
}

#[derive(Copy, Clone)]
enum GradientVariant {
    Linear {
        x1: Option<Length>,
        y1: Option<Length>,
        x2: Option<Length>,
        y2: Option<Length>,
    },

    Radial {
        cx: Option<Length>,
        cy: Option<Length>,
        r: Option<Length>,
        fx: Option<Length>,
        fy: Option<Length>,
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
            spread: Some(SpreadMethod::default()),
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
        self.units.is_some()
            && self.affine.is_some()
            && self.spread.is_some()
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

    fn add_color_stop(&mut self, mut offset: f64, rgba: cssparser::RGBA, opacity: UnitInterval) {
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

            stops.push(ColorStop {
                offset,
                rgba,
                opacity,
            });
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
            x1: Some(Length::parse_str("0%", LengthDir::Horizontal).unwrap()),
            y1: Some(Length::parse_str("0%", LengthDir::Vertical).unwrap()),
            x2: Some(Length::parse_str("100%", LengthDir::Horizontal).unwrap()),
            y2: Some(Length::parse_str("0%", LengthDir::Vertical).unwrap()),
        }
    }

    fn default_radial() -> Self {
        // https://www.w3.org/TR/SVG/pservers.html#RadialGradients

        GradientVariant::Radial {
            cx: Some(Length::parse_str("50%", LengthDir::Horizontal).unwrap()),
            cy: Some(Length::parse_str("50%", LengthDir::Vertical).unwrap()),
            r: Some(Length::parse_str("50%", LengthDir::Both).unwrap()),

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

        node.children()
            .into_iter()
            // just ignore this child; we are only interested in gradient stops
            .filter(|child| child.get_type() == NodeType::Stop)
            // don't add any more stops, Eq to break in for-loop
            .take_while(|child| {
                let in_error = child.is_in_error();

                if in_error {
                    rsvg_log!(
                        "(not using gradient stop {} because it is in error)",
                        child.get_human_readable_name()
                    );
                }

                !in_error
            })
            .for_each(|child| {
                child.with_impl(|stop: &NodeStop| {
                    let cascaded = child.get_cascaded_values();
                    let values = cascaded.get();

                    let rgba = match values.stop_color {
                        StopColor(cssparser::Color::CurrentColor) => values.color.0,
                        StopColor(cssparser::Color::RGBA(ref rgba)) => *rgba,
                    };
                    self.add_color_stop(stop.get_offset(), rgba, values.stop_opacity.0);
                })
            });
    }

    fn add_color_stop(&mut self, offset: f64, rgba: cssparser::RGBA, opacity: UnitInterval) {
        self.common.add_color_stop(offset, rgba, opacity);
    }

    fn add_color_stops_to_pattern(&self, pattern: &mut cairo::Gradient, opacity: &UnitInterval) {
        if let Some(stops) = self.common.stops.as_ref() {
            for stop in stops {
                let &UnitInterval(o) = opacity;
                let UnitInterval(stop_opacity) = stop.opacity;

                pattern.add_color_stop_rgba(
                    stop.offset,
                    f64::from(stop.rgba.red_f32()),
                    f64::from(stop.rgba.green_f32()),
                    f64::from(stop.rgba.blue_f32()),
                    f64::from(stop.rgba.alpha_f32()) * stop_opacity * o,
                );
            }
        }
    }

    fn bounds_are_valid(&self, bbox: &BoundingBox) -> bool {
        if self.common.units == Some(GradientUnits(CoordUnits::UserSpaceOnUse)) {
            true
        } else {
            bbox.rect.map_or(false, |r| !r.is_empty())
        }
    }
}

fn acquire_gradient<'a>(draw_ctx: &'a mut DrawingCtx<'_>, name: &str) -> Option<AcquiredNode> {
    if let Some(acquired) = draw_ctx.get_acquired_node(name) {
        let node_type = acquired.get().get_type();

        if node_type == NodeType::LinearGradient || node_type == NodeType::RadialGradient {
            return Some(acquired);
        }
    }

    rsvg_log!("element \"{}\" does not exist or is not a gradient", name);

    None
}

fn resolve_gradient(gradient: &Gradient, draw_ctx: &mut DrawingCtx<'_>) -> Gradient {
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
    draw_ctx: &mut DrawingCtx<'_>,
    pattern: &mut P,
    bbox: &BoundingBox,
    opacity: &UnitInterval,
) {
    let cr = draw_ctx.get_cairo_context();

    let mut affine = gradient.common.affine.unwrap();

    let units = gradient.common.units.unwrap();

    if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
        let bbox_rect = bbox.rect.unwrap();
        let bbox_matrix = cairo::Matrix::new(
            bbox_rect.width,
            0.0,
            0.0,
            bbox_rect.height,
            bbox_rect.x,
            bbox_rect.y,
        );
        affine = cairo::Matrix::multiply(&affine, &bbox_matrix);
    }

    affine.invert();
    pattern.set_matrix(affine);
    pattern.set_extend(cairo::enums::Extend::from(
        gradient.common.spread.unwrap_or_default(),
    ));

    gradient.add_color_stops_to_pattern(pattern, opacity);

    cr.set_source(pattern);
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
fn fix_focus_point(fx: f64, fy: f64, cx: f64, cy: f64, radius: f64) -> (f64, f64) {
    // Easy case first: the focus point is inside the circle

    if (fx - cx) * (fx - cx) + (fy - cy) * (fy - cy) <= radius * radius {
        return (fx, fy);
    }

    // Hard case: focus point is outside the circle.
    // Find the vector from the origin to (fx, fy)

    let mut dx = fx - cx;
    let mut dy = fy - cy;

    // Find the vector's magnitude
    let mag = (dx * dx + dy * dy).sqrt();

    // Normalize the vector to have a magnitude equal to radius
    let scale = mag / radius;

    dx /= scale;
    dy /= scale;

    // Translate back to (cx, cy) and we are done!

    (cx + dx, cy + dy)
}

fn set_pattern_on_draw_context(
    gradient: &Gradient,
    values: &ComputedValues,
    draw_ctx: &mut DrawingCtx<'_>,
    opacity: &UnitInterval,
    bbox: &BoundingBox,
) {
    assert!(gradient.is_resolved());

    let units = gradient.common.units.unwrap();
    let params = if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
        draw_ctx.push_view_box(1.0, 1.0)
    } else {
        draw_ctx.get_view_params()
    };

    match gradient.variant {
        GradientVariant::Linear { x1, y1, x2, y2 } => {
            let mut pattern = cairo::LinearGradient::new(
                x1.as_ref().unwrap().normalize(values, &params),
                y1.as_ref().unwrap().normalize(values, &params),
                x2.as_ref().unwrap().normalize(values, &params),
                y2.as_ref().unwrap().normalize(values, &params),
            );

            set_common_on_pattern(gradient, draw_ctx, &mut pattern, bbox, opacity)
        }

        GradientVariant::Radial { cx, cy, r, fx, fy } => {
            let n_cx = cx.as_ref().unwrap().normalize(values, &params);
            let n_cy = cy.as_ref().unwrap().normalize(values, &params);
            let n_r = r.as_ref().unwrap().normalize(values, &params);
            let n_fx = fx.as_ref().unwrap().normalize(values, &params);
            let n_fy = fy.as_ref().unwrap().normalize(values, &params);

            let (new_fx, new_fy) = fix_focus_point(n_fx, n_fy, n_cx, n_cy, n_r);
            let mut pattern = cairo::RadialGradient::new(new_fx, new_fy, 0.0, n_cx, n_cy, n_r);

            set_common_on_pattern(gradient, draw_ctx, &mut pattern, bbox, opacity)
        }
    }
}

pub struct NodeGradient {
    gradient: RefCell<Gradient>,
}

impl NodeGradient {
    pub fn new_linear() -> NodeGradient {
        NodeGradient {
            gradient: RefCell::new(Gradient {
                common: GradientCommon::unresolved(),
                variant: GradientVariant::unresolved_linear(),
            }),
        }
    }

    pub fn new_radial() -> NodeGradient {
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
    fn set_atts(
        &self,
        node: &RsvgNode,
        _: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
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
                    g.common.units = Some(parse("gradientUnits", value, ())?)
                }

                Attribute::GradientTransform => {
                    g.common.affine = Some(parse("gradientTransform", value, ())?)
                }

                Attribute::SpreadMethod => {
                    g.common.spread = Some(parse("spreadMethod", value, ())?)
                }

                Attribute::XlinkHref => g.common.fallback = Some(value.to_owned()),

                // Attributes specific to each gradient type.  The defaults mandated by the spec
                // are in GradientVariant::resolve_from_defaults()
                Attribute::X1 => x1 = Some(parse("x1", value, LengthDir::Horizontal)?),
                Attribute::Y1 => y1 = Some(parse("y1", value, LengthDir::Vertical)?),
                Attribute::X2 => x2 = Some(parse("x2", value, LengthDir::Horizontal)?),
                Attribute::Y2 => y2 = Some(parse("y2", value, LengthDir::Vertical)?),

                Attribute::Cx => cx = Some(parse("cx", value, LengthDir::Horizontal)?),
                Attribute::Cy => cy = Some(parse("cy", value, LengthDir::Vertical)?),
                Attribute::R => r = Some(parse("r", value, LengthDir::Both)?),
                Attribute::Fx => fx = Some(parse("fx", value, LengthDir::Horizontal)?),
                Attribute::Fy => fy = Some(parse("fy", value, LengthDir::Vertical)?),

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
}

pub fn gradient_resolve_fallbacks_and_set_pattern(
    node: &RsvgNode,
    draw_ctx: &mut DrawingCtx<'_>,
    opacity: &UnitInterval,
    bbox: &BoundingBox,
) -> bool {
    assert!(
        node.get_type() == NodeType::LinearGradient || node.get_type() == NodeType::RadialGradient
    );

    let mut did_set_gradient = false;

    node.with_impl(|node_gradient: &NodeGradient| {
        let gradient = node_gradient.get_gradient_with_color_stops_from_node(node);
        let resolved = resolve_gradient(&gradient, draw_ctx);

        if resolved.bounds_are_valid(bbox) {
            let cascaded = node.get_cascaded_values();
            let values = cascaded.get();
            set_pattern_on_draw_context(&resolved, values, draw_ctx, opacity, bbox);
        }

        did_set_gradient = true;
    });

    did_set_gradient
}

#[cfg(test)]
mod tests {
    use super::*;
    use float_eq_cairo::ApproxEqCairo;

    #[test]
    fn parses_spread_method() {
        assert_eq!(SpreadMethod::parse_str("pad", ()), Ok(SpreadMethod::Pad));
        assert_eq!(
            SpreadMethod::parse_str("reflect", ()),
            Ok(SpreadMethod::Reflect)
        );
        assert_eq!(
            SpreadMethod::parse_str("repeat", ()),
            Ok(SpreadMethod::Repeat)
        );
        assert!(SpreadMethod::parse_str("foobar", ()).is_err());
    }

    fn assert_tuples_equal(a: &(f64, f64), b: &(f64, f64)) {
        assert_approx_eq_cairo!(a.0, b.0);
        assert_approx_eq_cairo!(a.1, b.1);
    }

    #[test]
    fn fixes_focus_point() {
        // inside the circle
        assert_tuples_equal(&fix_focus_point(1.0, 1.0, 2.0, 1.0, 3.0), &(1.0, 1.0));

        // on the edge
        assert_tuples_equal(&fix_focus_point(1.0, 1.0, 2.0, 1.0, 2.0), &(1.0, 1.0));

        // outside the circle
        assert_tuples_equal(&fix_focus_point(1.0, 1.0, 3.0, 1.0, 1.0), &(2.0, 1.0));
    }

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
