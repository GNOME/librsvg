use cairo::{self, MatrixTrait};
use cssparser::{self, CowRcStr, Parser, Token};
use markup5ever::local_name;

use std::cell::RefCell;

use crate::allowed_url::Fragment;
use crate::bbox::*;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{AcquiredNode, DrawingCtx, NodeStack};
use crate::error::*;
use crate::length::*;
use crate::node::{CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::paint_server::PaintSource;
use crate::parsers::{Parse, ParseError, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::property_defs::StopColor;
use crate::rect::RectangleExt;
use crate::stop::*;
use crate::unit_interval::UnitInterval;

#[derive(Copy, Clone)]
pub struct ColorStop {
    pub offset: UnitInterval,
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
    type Err = ValueErrorKind;

    fn parse(parser: &mut Parser<'_, '_>) -> Result<SpreadMethod, ValueErrorKind> {
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

impl From<SpreadMethod> for cairo::Extend {
    fn from(s: SpreadMethod) -> cairo::Extend {
        match s {
            SpreadMethod::Pad => cairo::Extend::Pad,
            SpreadMethod::Reflect => cairo::Extend::Reflect,
            SpreadMethod::Repeat => cairo::Extend::Repeat,
        }
    }
}

// Any of the attributes in gradient elements may be omitted.  In turn, the missing
// ones can be inherited from the gradient referenced by its "fallback" IRI.  We
// represent these possibly-missing attributes as Option<foo>.
#[derive(Clone)]
pub struct GradientCommon {
    pub units: Option<GradientUnits>,
    pub affine: Option<cairo::Matrix>,
    pub spread: Option<SpreadMethod>,
    pub fallback: Option<Fragment>,
    pub stops: Option<Vec<ColorStop>>,
}

#[derive(Copy, Clone)]
pub enum GradientVariant {
    Linear {
        x1: Option<LengthHorizontal>,
        y1: Option<LengthVertical>,
        x2: Option<LengthHorizontal>,
        y2: Option<LengthVertical>,
    },

    Radial {
        cx: Option<LengthHorizontal>,
        cy: Option<LengthVertical>,
        r: Option<LengthBoth>,
        fx: Option<LengthHorizontal>,
        fy: Option<LengthVertical>,
    },
}

#[derive(Clone)]
pub struct Gradient {
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

        self.fallback = fallback.fallback.clone();
    }

    fn add_color_stop(
        &mut self,
        offset: UnitInterval,
        rgba: cssparser::RGBA,
        opacity: UnitInterval,
    ) {
        if self.stops.is_none() {
            self.stops = Some(Vec::<ColorStop>::new());
        }

        if let Some(ref mut stops) = self.stops {
            let last_offset = if !stops.is_empty() {
                stops[stops.len() - 1].offset
            } else {
                UnitInterval(0.0)
            };

            let offset = if offset > last_offset {
                offset
            } else {
                last_offset
            };

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
            x1: Some(LengthHorizontal::parse_str("0%").unwrap()),
            y1: Some(LengthVertical::parse_str("0%").unwrap()),
            x2: Some(LengthHorizontal::parse_str("100%").unwrap()),
            y2: Some(LengthVertical::parse_str("0%").unwrap()),
        }
    }

    fn default_radial() -> Self {
        // https://www.w3.org/TR/SVG/pservers.html#RadialGradients

        GradientVariant::Radial {
            cx: Some(LengthHorizontal::parse_str("50%").unwrap()),
            cy: Some(LengthVertical::parse_str("50%").unwrap()),
            r: Some(LengthBoth::parse_str("50%").unwrap()),

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
            node.borrow().get_type() == NodeType::LinearGradient
                || node.borrow().get_type() == NodeType::RadialGradient
        );

        for child in node
            .children()
            .filter(|child| child.borrow().get_type() == NodeType::Stop)
        {
            if child.borrow().is_in_error() {
                rsvg_log!("(not using gradient stop {} because it is in error)", child);
            } else {
                let offset = child.borrow().get_impl::<NodeStop>().get_offset();
                let cascaded = CascadedValues::new_from_node(&child);
                let values = cascaded.get();
                let rgba = match values.stop_color {
                    StopColor(cssparser::Color::CurrentColor) => values.color.0,
                    StopColor(cssparser::Color::RGBA(ref rgba)) => *rgba,
                };

                self.add_color_stop(offset, rgba, values.stop_opacity.0);
            }
        }
    }

    fn add_color_stop(
        &mut self,
        offset: UnitInterval,
        rgba: cssparser::RGBA,
        opacity: UnitInterval,
    ) {
        self.common.add_color_stop(offset, rgba, opacity);
    }

    fn add_color_stops_to_pattern<T, G: cairo::Gradient<PatternType = T>>(
        &self,
        pattern: &mut G,
        opacity: &UnitInterval,
    ) {
        if let Some(stops) = self.common.stops.as_ref() {
            for stop in stops {
                let UnitInterval(stop_offset) = stop.offset;
                let &UnitInterval(o) = opacity;
                let UnitInterval(stop_opacity) = stop.opacity;

                pattern.add_color_stop_rgba(
                    stop_offset,
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

fn acquire_gradient<'a>(
    draw_ctx: &'a mut DrawingCtx,
    name: Option<&Fragment>,
) -> Option<AcquiredNode> {
    name.and_then(move |fragment| draw_ctx.acquired_nodes().get_node(fragment))
        .and_then(|acquired| {
            let node_type = acquired.get().borrow().get_type();

            if node_type == NodeType::LinearGradient || node_type == NodeType::RadialGradient {
                Some(acquired)
            } else {
                None
            }
        })
}

fn set_common_on_pattern<P: cairo::PatternTrait + cairo::Gradient>(
    gradient: &Gradient,
    pattern: &mut P,
    bbox: &BoundingBox,
    opacity: &UnitInterval,
) {
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
    pattern.set_extend(cairo::Extend::from(
        gradient.common.spread.unwrap_or_default(),
    ));

    gradient.add_color_stops_to_pattern(pattern, opacity);
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

impl PaintSource for NodeGradient {
    type Source = Gradient;

    fn resolve(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
        bbox: &BoundingBox,
    ) -> Result<Option<Self::Source>, RenderingError> {
        let mut result = get_gradient_with_color_stops_from_node(node);
        let mut stack = NodeStack::new();

        while !result.is_resolved() {
            if let Some(acquired) = acquire_gradient(draw_ctx, result.common.fallback.as_ref()) {
                let a_node = acquired.get();

                if stack.contains(a_node) {
                    rsvg_log!("circular reference in gradient {}", node);
                    return Err(RenderingError::CircularReference);
                }

                let fallback = get_gradient_with_color_stops_from_node(&a_node);
                result.resolve_from_fallback(&fallback);

                stack.push(a_node);
                continue;
            }

            result.resolve_from_defaults();
        }

        if result.bounds_are_valid(bbox) {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    fn set_pattern_on_draw_context(
        &self,
        gradient: &Self::Source,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: &UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
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

                let cr = draw_ctx.get_cairo_context();
                set_common_on_pattern(gradient, &mut pattern, bbox, opacity);
                cr.set_source(&cairo::Pattern::LinearGradient(pattern));
            }

            GradientVariant::Radial { cx, cy, r, fx, fy } => {
                let n_cx = cx.as_ref().unwrap().normalize(values, &params);
                let n_cy = cy.as_ref().unwrap().normalize(values, &params);
                let n_r = r.as_ref().unwrap().normalize(values, &params);
                let n_fx = fx.as_ref().unwrap().normalize(values, &params);
                let n_fy = fy.as_ref().unwrap().normalize(values, &params);

                let (new_fx, new_fy) = fix_focus_point(n_fx, n_fy, n_cx, n_cy, n_r);
                let mut pattern = cairo::RadialGradient::new(new_fx, new_fy, 0.0, n_cx, n_cy, n_r);

                let cr = draw_ctx.get_cairo_context();
                set_common_on_pattern(gradient, &mut pattern, bbox, opacity);
                cr.set_source(&cairo::Pattern::RadialGradient(pattern));
            }
        }

        Ok(true)
    }
}

fn get_gradient_with_color_stops_from_node(node: &RsvgNode) -> Gradient {
    let mut gradient = node
        .borrow()
        .get_impl::<NodeGradient>()
        .gradient
        .borrow()
        .clone();
    gradient.add_color_stops_from_node(node);
    gradient
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
}

impl NodeTrait for NodeGradient {
    fn set_atts(&self, node: &RsvgNode, pbag: &PropertyBag<'_>) -> NodeResult {
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

        for (attr, value) in pbag.iter() {
            match attr {
                // Attributes common to linear and radial gradients
                local_name!("gradientUnits") => g.common.units = Some(attr.parse(value)?),
                local_name!("gradientTransform") => g.common.affine = Some(attr.parse(value)?),
                local_name!("spreadMethod") => g.common.spread = Some(attr.parse(value)?),
                local_name!("xlink:href") => {
                    g.common.fallback = Some(Fragment::parse(value).attribute(attr)?)
                }

                // Attributes specific to each gradient type.  The defaults mandated by the spec
                // are in GradientVariant::resolve_from_defaults()
                local_name!("x1") => x1 = Some(attr.parse(value)?),
                local_name!("y1") => y1 = Some(attr.parse(value)?),
                local_name!("x2") => x2 = Some(attr.parse(value)?),
                local_name!("y2") => y2 = Some(attr.parse(value)?),

                local_name!("cx") => cx = Some(attr.parse(value)?),
                local_name!("cy") => cy = Some(attr.parse(value)?),
                local_name!("r") => r = Some(attr.parse(value)?),
                local_name!("fx") => fx = Some(attr.parse(value)?),
                local_name!("fy") => fy = Some(attr.parse(value)?),

                _ => (),
            }
        }

        match node.borrow().get_type() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_eq_cairo::ApproxEqCairo;

    #[test]
    fn parses_spread_method() {
        assert_eq!(SpreadMethod::parse_str("pad"), Ok(SpreadMethod::Pad));
        assert_eq!(
            SpreadMethod::parse_str("reflect"),
            Ok(SpreadMethod::Reflect)
        );
        assert_eq!(SpreadMethod::parse_str("repeat"), Ok(SpreadMethod::Repeat));
        assert!(SpreadMethod::parse_str("foobar").is_err());
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
