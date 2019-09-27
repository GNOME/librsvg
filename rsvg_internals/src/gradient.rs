use cairo;
use cssparser::{self, CowRcStr, Parser, Token};
use markup5ever::local_name;

use crate::allowed_url::Fragment;
use crate::bbox::*;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{AcquiredNode, DrawingCtx, NodeStack, ViewParams};
use crate::error::*;
use crate::length::*;
use crate::node::{CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::paint_server::{PaintSource, ResolvedPaintSource};
use crate::parsers::{Parse, ParseError, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::property_defs::StopColor;
use crate::rect::RectangleExt;
use crate::unit_interval::UnitInterval;

#[derive(Copy, Clone)]
struct ColorStop {
    offset: UnitInterval,
    rgba: cssparser::RGBA,
    opacity: UnitInterval,
}

coord_units!(GradientUnits, CoordUnits::ObjectBoundingBox);

#[derive(Debug, Copy, Clone, PartialEq)]
enum SpreadMethod {
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

macro_rules! fallback_to (
    ($dest:expr, $default:expr) => (
        $dest = $dest.take ().or_else (|| $default)
    );
);

#[derive(Clone)]
struct UnresolvedCommon {
    units: Option<GradientUnits>,
    affine: Option<cairo::Matrix>,
    spread: Option<SpreadMethod>,
    stops: Option<Vec<ColorStop>>,
}

struct Common {
    units: GradientUnits,
    affine: cairo::Matrix,
    spread: SpreadMethod,
    stops: Vec<ColorStop>,
}

impl Common {
    fn set_on_cairo_pattern(
        &self,
        pattern: &cairo::Gradient,
        bbox: &BoundingBox,
        opacity: &UnitInterval,
    ) {
        let mut affine = self.affine;
        let units = self.units;

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
        pattern.set_extend(cairo::Extend::from(self.spread));

        self.add_color_stops_to_pattern(pattern, opacity);
    }

    fn add_color_stops_to_pattern(&self, pattern: &cairo::Gradient, opacity: &UnitInterval) {
        for stop in &self.stops {
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

impl UnresolvedCommon {
    fn new_without_stops(
        units: Option<GradientUnits>,
        affine: Option<cairo::Matrix>,
        spread: Option<SpreadMethod>,
    ) -> UnresolvedCommon {
        UnresolvedCommon {
            units,
            affine,
            spread,
            stops: None,
        }
    }

    fn to_resolved(self) -> Common {
        assert!(self.is_resolved());

        let UnresolvedCommon {
            units,
            affine,
            spread,
            stops,
            ..
        } = self;

        Common {
            units: units.unwrap(),
            affine: affine.unwrap(),
            spread: spread.unwrap(),
            stops: stops.unwrap(),
        }
    }

    fn clone_stops(&self) -> Option<Vec<ColorStop>> {
        if let Some(ref stops) = self.stops {
            Some(stops.clone())
        } else {
            None
        }
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

    fn add_color_stops_from_node(&mut self, node: &RsvgNode) {
        assert!(node.borrow().get_type() == NodeType::Gradient);

        for child_node in node.children()
        {
            let child = child_node.borrow();

            if child.get_type() != NodeType::Stop {
                continue;
            }

            let stop = child.get_impl::<NodeStop>();

            if child.is_in_error() {
                rsvg_log!("(not using gradient stop {} because it is in error)", child);
            } else {
                let offset = stop.get_offset();
                let cascaded = CascadedValues::new_from_node(&child_node);
                let values = cascaded.get();
                let rgba = match values.stop_color {
                    StopColor(cssparser::Color::CurrentColor) => values.color.0,
                    StopColor(cssparser::Color::RGBA(ref rgba)) => *rgba,
                };

                self.add_color_stop(offset, rgba, values.stop_opacity.0);
            }
        }
    }

    fn bounds_are_valid(&self, bbox: &BoundingBox) -> bool {
        if self.units == Some(GradientUnits(CoordUnits::UserSpaceOnUse)) {
            true
        } else {
            bbox.rect.map_or(false, |r| !r.is_empty())
        }
    }

    fn is_resolved(&self) -> bool {
        self.units.is_some()
            && self.affine.is_some()
            && self.spread.is_some()
            && self.stops.is_some()
    }

    fn resolve_from_fallback(&mut self, fallback: &Self) {
        fallback_to!(self.units, fallback.units);
        fallback_to!(self.affine, fallback.affine);
        fallback_to!(self.spread, fallback.spread);
        fallback_to!(self.stops, fallback.clone_stops());
    }

    fn resolve_from_defaults(&mut self) {
        fallback_to!(self.units, Some(GradientUnits::default()));
        fallback_to!(self.affine, Some(cairo::Matrix::identity()));
        fallback_to!(self.spread, Some(SpreadMethod::default()));
        fallback_to!(self.stops, Some(Vec::<ColorStop>::new()));
    }
}

#[derive(Copy, Clone, Default)]
struct UnresolvedLinear {
    x1: Option<LengthHorizontal>,
    y1: Option<LengthVertical>,
    x2: Option<LengthHorizontal>,
    y2: Option<LengthVertical>,
}

struct Linear {
    x1: LengthHorizontal,
    y1: LengthVertical,
    x2: LengthHorizontal,
    y2: LengthVertical,
}

impl Linear {
    fn to_cairo_gradient(
        &self,
        values: &ComputedValues,
        params: &ViewParams,
    ) -> cairo::LinearGradient {
        cairo::LinearGradient::new(
            self.x1.normalize(values, params),
            self.y1.normalize(values, params),
            self.x2.normalize(values, params),
            self.y2.normalize(values, params),
        )
    }
}

impl UnresolvedLinear {
    fn set_atts(&mut self, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("x1") => self.x1 = Some(attr.parse(value)?),
                local_name!("y1") => self.y1 = Some(attr.parse(value)?),
                local_name!("x2") => self.x2 = Some(attr.parse(value)?),
                local_name!("y2") => self.y2 = Some(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn to_resolved(self) -> Linear {
        assert!(self.is_resolved());

        let UnresolvedLinear { x1, y1, x2, y2 } = self;

        Linear {
            x1: x1.unwrap(),
            y1: y1.unwrap(),
            x2: x2.unwrap(),
            y2: y2.unwrap(),
        }
    }

    fn is_resolved(&self) -> bool {
        self.x1.is_some() && self.y1.is_some() && self.x2.is_some() && self.y2.is_some()
    }

    fn resolve_from_fallback(&mut self, fallback: &Self) {
        fallback_to!(self.x1, fallback.x1);
        fallback_to!(self.y1, fallback.y1);
        fallback_to!(self.x2, fallback.x2);
        fallback_to!(self.y2, fallback.y2);
    }

    // https://www.w3.org/TR/SVG/pservers.html#LinearGradients
    fn resolve_from_defaults(&mut self) {
        fallback_to!(self.x1, Some(LengthHorizontal::parse_str("0%").unwrap()));
        fallback_to!(self.y1, Some(LengthVertical::parse_str("0%").unwrap()));
        fallback_to!(self.x2, Some(LengthHorizontal::parse_str("100%").unwrap()));
        fallback_to!(self.y2, Some(LengthVertical::parse_str("0%").unwrap()));
    }
}

#[derive(Copy, Clone, Default)]
struct UnresolvedRadial {
    cx: Option<LengthHorizontal>,
    cy: Option<LengthVertical>,
    r: Option<LengthBoth>,
    fx: Option<LengthHorizontal>,
    fy: Option<LengthVertical>,
}

struct Radial {
    cx: LengthHorizontal,
    cy: LengthVertical,
    r: LengthBoth,
    fx: LengthHorizontal,
    fy: LengthVertical,
}

impl Radial {
    fn to_cairo_gradient(
        &self,
        values: &ComputedValues,
        params: &ViewParams,
    ) -> cairo::RadialGradient {
        let n_cx = self.cx.normalize(values, params);
        let n_cy = self.cy.normalize(values, params);
        let n_r = self.r.normalize(values, params);
        let n_fx = self.fx.normalize(values, params);
        let n_fy = self.fy.normalize(values, params);
        let (new_fx, new_fy) = fix_focus_point(n_fx, n_fy, n_cx, n_cy, n_r);

        cairo::RadialGradient::new(new_fx, new_fy, 0.0, n_cx, n_cy, n_r)
    }
}

impl UnresolvedRadial {
    fn set_atts(&mut self, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("cx") => self.cx = Some(attr.parse(value)?),
                local_name!("cy") => self.cy = Some(attr.parse(value)?),
                local_name!("r") => self.r = Some(attr.parse(value)?),
                local_name!("fx") => self.fx = Some(attr.parse(value)?),
                local_name!("fy") => self.fy = Some(attr.parse(value)?),
                _ => (),
            }
        }

        Ok(())
    }

    fn to_resolved(self) -> Radial {
        assert!(self.is_resolved());

        let UnresolvedRadial { cx, cy, r, fx, fy } = self;

        Radial {
            cx: cx.unwrap(),
            cy: cy.unwrap(),
            r: r.unwrap(),
            fx: fx.unwrap(),
            fy: fy.unwrap(),
        }
    }

    fn is_resolved(&self) -> bool {
        self.cx.is_some()
            && self.cy.is_some()
            && self.r.is_some()
            && self.fx.is_some()
            && self.fy.is_some()
    }

    fn resolve_from_fallback(&mut self, fallback: &Self) {
        fallback_to!(self.cx, fallback.cx);
        fallback_to!(self.cy, fallback.cy);
        fallback_to!(self.r, fallback.r);
        fallback_to!(self.fx, fallback.fx);
        fallback_to!(self.fy, fallback.fy);
    }

    // https://www.w3.org/TR/SVG/pservers.html#RadialGradients
    fn resolve_from_defaults(&mut self) {
        fallback_to!(self.cx, Some(LengthHorizontal::parse_str("50%").unwrap()));
        fallback_to!(self.cy, Some(LengthVertical::parse_str("50%").unwrap()));
        fallback_to!(self.r, Some(LengthBoth::parse_str("50%").unwrap()));

        // fx and fy fall back to the presentational value of cx and cy
        fallback_to!(self.fx, self.cx);
        fallback_to!(self.fy, self.cy);
    }
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

#[derive(Default)]
pub struct NodeStop {
    offset: UnitInterval,
}

impl NodeStop {
    pub fn get_offset(&self) -> UnitInterval {
        self.offset
    }
}

fn validate_offset(length: LengthBoth) -> Result<LengthBoth, ValueErrorKind> {
    match length.unit() {
        LengthUnit::Px | LengthUnit::Percent => Ok(length),
        _ => Err(ValueErrorKind::Value(
            "stop offset must be in default or percent units".to_string(),
        )),
    }
}

impl NodeTrait for NodeStop {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("offset") => {
                    self.offset = attr
                        .parse_and_validate(value, validate_offset)
                        .map(|l| UnitInterval::clamp(l.length()))?
                }
                _ => (),
            }
        }

        Ok(())
    }
}

#[derive(Copy, Clone)]
enum UnresolvedVariant {
    Linear(UnresolvedLinear),
    Radial(UnresolvedRadial),
}

enum Variant {
    Linear(Linear),
    Radial(Radial),
}

impl UnresolvedVariant {
    fn to_resolved(self) -> Variant {
        match self {
            UnresolvedVariant::Linear(v) => Variant::Linear(v.to_resolved()),
            UnresolvedVariant::Radial(v) => Variant::Radial(v.to_resolved()),
        }
    }

    fn is_resolved(&self) -> bool {
        match *self {
            UnresolvedVariant::Linear(v) => v.is_resolved(),
            UnresolvedVariant::Radial(v) => v.is_resolved(),
        }
    }

    fn resolve_from_fallback(&mut self, fallback: &UnresolvedVariant) {
        match (self, fallback) {
            (&mut UnresolvedVariant::Linear(ref mut v), &UnresolvedVariant::Linear(ref f)) => {
                v.resolve_from_fallback(f)
            }

            (&mut UnresolvedVariant::Radial(ref mut v), &UnresolvedVariant::Radial(ref f)) => {
                v.resolve_from_fallback(f)
            }

            _ => (), // If variants are of different types, then nothing to resolve
        }
    }

    fn resolve_from_defaults(&mut self) {
        match *self {
            UnresolvedVariant::Linear(ref mut v) => v.resolve_from_defaults(),
            UnresolvedVariant::Radial(ref mut v) => v.resolve_from_defaults(),
        }
    }
}

pub struct NodeGradient {
    units: Option<GradientUnits>,
    affine: Option<cairo::Matrix>,
    spread: Option<SpreadMethod>,

    variant: UnresolvedVariant,

    fallback: Option<Fragment>,
}

struct UnresolvedGradient {
    common: UnresolvedCommon,
    variant: UnresolvedVariant,
}

pub struct Gradient {
    common: Common,
    variant: Variant,
}

impl UnresolvedGradient {
    fn to_resolved(self) -> Gradient {
        let UnresolvedGradient {
            common, variant, ..
        } = self;

        match variant {
            UnresolvedVariant::Linear(_) => Gradient {
                common: common.to_resolved(),
                variant: variant.to_resolved(),
            },

            UnresolvedVariant::Radial(_) => Gradient {
                common: common.to_resolved(),
                variant: variant.to_resolved(),
            },
        }
    }

    fn is_resolved(&self) -> bool {
        self.common.is_resolved() && self.variant.is_resolved()
    }

    fn resolve_from_fallback(&mut self, fallback: &UnresolvedGradient) {
        self.common.resolve_from_fallback(&fallback.common);
        self.variant.resolve_from_fallback(&fallback.variant);
    }

    fn resolve_from_defaults(&mut self) {
        self.common.resolve_from_defaults();
        self.variant.resolve_from_defaults();
    }
}

impl NodeGradient {
    pub fn new_linear() -> NodeGradient {
        NodeGradient {
            units: Default::default(),
            affine: Default::default(),
            spread: Default::default(),
            variant: UnresolvedVariant::Linear(UnresolvedLinear::default()),
            fallback: Default::default(),
        }
    }

    pub fn new_radial() -> NodeGradient {
        NodeGradient {
            units: Default::default(),
            affine: Default::default(),
            spread: Default::default(),
            variant: UnresolvedVariant::Radial(UnresolvedRadial::default()),
            fallback: Default::default(),
        }
    }

    fn get_unresolved(&self, node: &RsvgNode) -> (UnresolvedGradient, Option<Fragment>) {
        let mut common = UnresolvedCommon::new_without_stops(self.units, self.affine, self.spread);

        common.add_color_stops_from_node(node);

        (
            UnresolvedGradient {
                common,
                variant: self.variant,
            },
            self.fallback.clone(),
        )
    }
}

impl NodeTrait for NodeGradient {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("gradientUnits") => self.units = Some(attr.parse(value)?),
                local_name!("gradientTransform") => self.affine = Some(attr.parse(value)?),
                local_name!("spreadMethod") => self.spread = Some(attr.parse(value)?),
                _ => (),
            }
        }

        match self.variant {
            UnresolvedVariant::Linear(ref mut v) => v.set_atts(pbag)?,
            UnresolvedVariant::Radial(ref mut v) => v.set_atts(pbag)?,
        }

        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("xlink:href") => {
                    self.fallback = Some(Fragment::parse(value).attribute(attr)?)
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl PaintSource for NodeGradient {
    type Resolved = Gradient;

    fn resolve(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
        bbox: &BoundingBox,
    ) -> Result<Option<Self::Resolved>, RenderingError> {
        let (mut result, mut fallback) = self.get_unresolved(node);

        let mut stack = NodeStack::new();

        while !result.is_resolved() {
            if let Some(acquired) = acquire_gradient(draw_ctx, fallback.as_ref()) {
                let a_node = acquired.get();

                if stack.contains(a_node) {
                    rsvg_log!("circular reference in gradient {}", node);
                    return Err(RenderingError::CircularReference);
                }

                let (a_gradient, next_fallback) = a_node
                    .borrow()
                    .get_impl::<NodeGradient>()
                    .get_unresolved(a_node);

                result.resolve_from_fallback(&a_gradient);
                fallback = next_fallback;

                stack.push(a_node);
            } else {
                result.resolve_from_defaults();
            }
        }

        if result.common.bounds_are_valid(bbox) {
            Ok(Some(result.to_resolved()))
        } else {
            Ok(None)
        }
    }
}

impl ResolvedPaintSource for Gradient {
    fn set_pattern_on_draw_context(
        self,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: &UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        let units = self.common.units;
        let params = if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

        let p = match self.variant {
            Variant::Linear(v) => {
                let g = v.to_cairo_gradient(values, &params);
                cairo::Gradient::clone(&g)
            }

            Variant::Radial(v) => {
                let g = v.to_cairo_gradient(values, &params);
                cairo::Gradient::clone(&g)
            }
        };

        self.common.set_on_cairo_pattern(&p, bbox, opacity);
        let cr = draw_ctx.get_cairo_context();
        cr.set_source(&p);

        Ok(true)
    }
}

fn acquire_gradient<'a>(
    draw_ctx: &'a mut DrawingCtx,
    name: Option<&Fragment>,
) -> Option<AcquiredNode> {
    name.and_then(move |fragment| draw_ctx.acquired_nodes().get_node(fragment))
        .and_then(|acquired| {
            let node_type = acquired.get().borrow().get_type();

            if node_type == NodeType::Gradient {
                Some(acquired)
            } else {
                None
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_eq_cairo::ApproxEqCairo;
    use crate::node::{NodeData, NodeType, RsvgNode};

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
        let node = RsvgNode::new(NodeData::new(
            NodeType::Gradient,
            local_name!("linearGradient"),
            None,
            None,
            Box::new(NodeGradient::new_linear())
        ));

        let borrow = node.borrow();
        let g = borrow.get_impl::<NodeGradient>();
        let (mut u, _) = g.get_unresolved(&node);
        u.resolve_from_defaults();
        assert!(u.is_resolved());

        let node = RsvgNode::new(NodeData::new(
            NodeType::Gradient,
            local_name!("radialGradient"),
            None,
            None,
            Box::new(NodeGradient::new_radial())
        ));

        let borrow = node.borrow();
        let g = borrow.get_impl::<NodeGradient>();
        let (mut u, _) = g.get_unresolved(&node);
        u.resolve_from_defaults();
        assert!(u.is_resolved());
    }
}
