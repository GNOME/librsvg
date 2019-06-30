use cairo::{self, MatrixTrait};
use cssparser::{self, CowRcStr, Parser, Token};
use markup5ever::local_name;

use std::cell::{Cell, RefCell};

use crate::allowed_url::Fragment;
use crate::bbox::*;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{AcquiredNode, DrawingCtx, NodeStack, ViewParams};
use crate::error::*;
use crate::length::*;
use crate::node::{CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::paint_server::{PaintSource, Resolve};
use crate::parsers::{Parse, ParseError, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::property_defs::StopColor;
use crate::rect::RectangleExt;
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

macro_rules! fallback_to (
    ($dest:expr, $default:expr) => (
        $dest = $dest.take ().or ($default)
    );
);

#[derive(Clone, Default)]
pub struct GradientCommon {
    pub units: Option<GradientUnits>,
    pub affine: Option<cairo::Matrix>,
    pub spread: Option<SpreadMethod>,
    pub fallback: Option<Fragment>,
    pub stops: Option<Vec<ColorStop>>,
}

impl Resolve for GradientCommon {
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

        self.fallback = fallback.fallback.clone();
    }

    fn resolve_from_defaults(&mut self) {
        fallback_to!(self.units, Some(GradientUnits::default()));
        fallback_to!(self.affine, Some(cairo::Matrix::identity()));
        fallback_to!(self.spread, Some(SpreadMethod::default()));
        fallback_to!(self.stops, Some(Vec::<ColorStop>::new()));
    }
}

impl GradientCommon {
    fn set_atts(&mut self, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("gradientUnits") => self.units = Some(attr.parse(value)?),
                local_name!("gradientTransform") => self.affine = Some(attr.parse(value)?),
                local_name!("spreadMethod") => self.spread = Some(attr.parse(value)?),
                local_name!("xlink:href") => {
                    self.fallback = Some(Fragment::parse(value).attribute(attr)?)
                }
                _ => (),
            }
        }

        Ok(())
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

    fn add_color_stops_to_pattern<T, G: cairo::Gradient<PatternType = T>>(
        &self,
        pattern: &mut G,
        opacity: &UnitInterval,
    ) {
        if let Some(stops) = self.stops.as_ref() {
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

    fn set_on_pattern<P: cairo::PatternTrait + cairo::Gradient>(
        &self,
        pattern: &mut P,
        bbox: &BoundingBox,
        opacity: &UnitInterval,
    ) {
        let mut affine = self.affine.unwrap();
        let units = self.units.unwrap();

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
        pattern.set_extend(cairo::Extend::from(self.spread.unwrap_or_default()));

        self.add_color_stops_to_pattern(pattern, opacity);
    }

    fn bounds_are_valid(&self, bbox: &BoundingBox) -> bool {
        if self.units == Some(GradientUnits(CoordUnits::UserSpaceOnUse)) {
            true
        } else {
            bbox.rect.map_or(false, |r| !r.is_empty())
        }
    }
}

#[derive(Copy, Clone, Default)]
pub struct GradientLinear {
    x1: Option<LengthHorizontal>,
    y1: Option<LengthVertical>,
    x2: Option<LengthHorizontal>,
    y2: Option<LengthVertical>,
}

impl Resolve for GradientLinear {
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

impl GradientLinear {
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

    fn to_cairo_gradient(
        &self,
        values: &ComputedValues,
        params: &ViewParams,
    ) -> cairo::LinearGradient {
        cairo::LinearGradient::new(
            self.x1.as_ref().unwrap().normalize(values, params),
            self.y1.as_ref().unwrap().normalize(values, params),
            self.x2.as_ref().unwrap().normalize(values, params),
            self.y2.as_ref().unwrap().normalize(values, params),
        )
    }
}

#[derive(Copy, Clone, Default)]
pub struct GradientRadial {
    cx: Option<LengthHorizontal>,
    cy: Option<LengthVertical>,
    r: Option<LengthBoth>,
    fx: Option<LengthHorizontal>,
    fy: Option<LengthVertical>,
}

impl Resolve for GradientRadial {
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

impl GradientRadial {
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

    fn to_cairo_gradient(
        &self,
        values: &ComputedValues,
        params: &ViewParams,
    ) -> cairo::RadialGradient {
        let n_cx = self.cx.as_ref().unwrap().normalize(values, params);
        let n_cy = self.cy.as_ref().unwrap().normalize(values, params);
        let n_r = self.r.as_ref().unwrap().normalize(values, params);
        let n_fx = self.fx.as_ref().unwrap().normalize(values, params);
        let n_fy = self.fy.as_ref().unwrap().normalize(values, params);
        let (new_fx, new_fy) = fix_focus_point(n_fx, n_fy, n_cx, n_cy, n_r);

        cairo::RadialGradient::new(new_fx, new_fy, 0.0, n_cx, n_cy, n_r)
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
    offset: Cell<UnitInterval>,
}

impl NodeStop {
    pub fn get_offset(&self) -> UnitInterval {
        self.offset.get()
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
    fn set_atts(&self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("offset") => {
                    self.offset.set(
                        attr.parse_and_validate(value, validate_offset)
                            .map(|l| UnitInterval::clamp(l.length()))?,
                    );
                }
                _ => (),
            }
        }

        Ok(())
    }
}

macro_rules! impl_node_trait {
    ($gradient_type:ty) => {
        impl NodeTrait for $gradient_type {
            fn set_atts(&self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
                let mut common = self.common.borrow_mut();
                common.set_atts(pbag)?;

                let mut variant = self.variant.borrow_mut();
                variant.set_atts(pbag)?;

                Ok(())
            }
        }
    };
}

macro_rules! impl_resolve {
    ($gradient_type:ty) => {
        impl Resolve for $gradient_type {
            fn is_resolved(&self) -> bool {
                self.common.borrow().is_resolved() && self.variant.borrow().is_resolved()
            }

            fn resolve_from_fallback(&mut self, fallback: &$gradient_type) {
                self.common
                    .borrow_mut()
                    .resolve_from_fallback(&fallback.common.borrow());
                self.variant
                    .borrow_mut()
                    .resolve_from_fallback(&fallback.variant.borrow());
            }

            fn resolve_from_defaults(&mut self) {
                self.common.borrow_mut().resolve_from_defaults();
                self.variant.borrow_mut().resolve_from_defaults();
            }
        }
    };
}

macro_rules! impl_paint_source_resolve {
    ($gradient:ty, $node_type:pat, $other_gradient:ty, $other_type:pat) => {
        fn resolve(
            &self,
            node: &RsvgNode,
            draw_ctx: &mut DrawingCtx,
            bbox: &BoundingBox,
        ) -> Result<Option<Self::Source>, RenderingError> {
            let mut result = self.clone();
            result.common.borrow_mut().add_color_stops_from_node(node);

            let mut stack = NodeStack::new();

            while !result.is_resolved() {
                let acquired = acquire_gradient(draw_ctx, result.common.borrow().fallback.as_ref());

                if let Some(acquired) = acquired {
                    let a_node = acquired.get();

                    if stack.contains(a_node) {
                        rsvg_log!("circular reference in gradient {}", node);
                        return Err(RenderingError::CircularReference);
                    }

                    match a_node.borrow().get_type() {
                        // Same type, resolve all attributes
                        $node_type => {
                            let fallback = a_node
                                .borrow()
                                .get_impl::<$gradient>()
                                .clone();
                            fallback.common.borrow_mut().add_color_stops_from_node(a_node);
                            result.resolve_from_fallback(&fallback);
                        }
                        // Other type of gradient, resolve common attributes
                        $other_type => {
                            let fallback = a_node
                                .borrow()
                                .get_impl::<$other_gradient>()
                                .clone();
                            fallback.common.borrow_mut().add_color_stops_from_node(a_node);
                            result.common.borrow_mut().resolve_from_fallback(&fallback.common.borrow());
                        }
                        _ => (),
                    }

                    stack.push(a_node);

                    continue;
                }

                result.resolve_from_defaults();
            }

            if result.common.borrow().bounds_are_valid(bbox) {
                Ok(Some(result))
            } else {
                Ok(None)
            }
        }
    };
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

#[derive(Clone, Default)]
pub struct NodeLinearGradient {
    pub common: RefCell<GradientCommon>,
    pub variant: RefCell<GradientLinear>,
}

impl_node_trait!(NodeLinearGradient);

impl_resolve!(NodeLinearGradient);

impl PaintSource for NodeLinearGradient {
    type Source = NodeLinearGradient;

    impl_paint_source_resolve!(
        NodeLinearGradient,
        NodeType::LinearGradient,
        NodeRadialGradient,
        NodeType::RadialGradient
    );

    fn set_pattern_on_draw_context(
        &self,
        gradient: &Self::Source,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: &UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        assert!(gradient.is_resolved());

        let units = gradient.common.borrow().units.unwrap();
        let params = if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

        let mut pattern = gradient.variant.borrow().to_cairo_gradient(values, &params);
        let cr = draw_ctx.get_cairo_context();
        gradient
            .common
            .borrow_mut()
            .set_on_pattern(&mut pattern, bbox, opacity);
        cr.set_source(&cairo::Pattern::LinearGradient(pattern));

        Ok(true)
    }
}

#[derive(Clone, Default)]
pub struct NodeRadialGradient {
    pub common: RefCell<GradientCommon>,
    pub variant: RefCell<GradientRadial>,
}

impl_node_trait!(NodeRadialGradient);

impl_resolve!(NodeRadialGradient);

impl PaintSource for NodeRadialGradient {
    type Source = NodeRadialGradient;

    impl_paint_source_resolve!(
        NodeRadialGradient,
        NodeType::RadialGradient,
        NodeLinearGradient,
        NodeType::LinearGradient
    );

    fn set_pattern_on_draw_context(
        &self,
        gradient: &Self::Source,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: &UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        assert!(gradient.is_resolved());

        let units = gradient.common.borrow().units.unwrap();
        let params = if units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

        let mut pattern = gradient.variant.borrow().to_cairo_gradient(values, &params);
        let cr = draw_ctx.get_cairo_context();
        gradient
            .common
            .borrow_mut()
            .set_on_pattern(&mut pattern, bbox, opacity);
        cr.set_source(&cairo::Pattern::RadialGradient(pattern));

        Ok(true)
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
        let mut l = NodeLinearGradient::default();
        l.resolve_from_defaults();
        assert!(l.is_resolved());

        let mut r = NodeRadialGradient::default();
        r.resolve_from_defaults();
        assert!(r.is_resolved());
    }
}
