use cairo;
use cssparser::{self, CowRcStr, Parser, Token};
use markup5ever::local_name;
use std::cell::RefCell;

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

/// Contents of a <stop> element for gradient color stops
#[derive(Copy, Clone)]
struct ColorStop {
    /// <stop offset="..."/>
    offset: UnitInterval,

    /// <stop stop-color="..."/>
    rgba: cssparser::RGBA,

    /// <stop stop-opacity="..."/>
    opacity: UnitInterval,
}

// gradientUnits attibute; its default is objectBoundingBox
coord_units!(GradientUnits, CoordUnits::ObjectBoundingBox);

/// spreadMethod attribute for gradients
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

/// Node for the <stop> element
#[derive(Default)]
pub struct NodeStop {
    /// <stop offset="..."/>
    offset: UnitInterval,

    // stop-color and stop-opacity are not attributes; they are properties, so
    // they go into property_defs.rs
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

/// Parameters specific to each gradient type, before being resolved.
/// These will be composed together with UnreseolvedVariant from fallback
/// nodes (referenced with e.g. <linearGradient xlink:href="#fallback">) to form
/// a final, resolved Variant.
#[derive(Copy, Clone)]
enum UnresolvedVariant {
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

/// Parameters specific to each gradient type, after resolving.
#[derive(Clone)]
enum Variant {
    Linear {
        x1: LengthHorizontal,
        y1: LengthVertical,
        x2: LengthHorizontal,
        y2: LengthVertical,
    },

    Radial {
        cx: LengthHorizontal,
        cy: LengthVertical,
        r: LengthBoth,
        fx: LengthHorizontal,
        fy: LengthVertical,
    },
}

impl UnresolvedVariant {
    fn to_resolved(self) -> Variant {
        assert!(self.is_resolved());

        match self {
            UnresolvedVariant::Linear { x1, y1, x2, y2 } => Variant::Linear {
                x1: x1.unwrap(),
                y1: y1.unwrap(),
                x2: x2.unwrap(),
                y2: y2.unwrap(),
            },

            UnresolvedVariant::Radial { cx, cy, r, fx, fy } => Variant::Radial {
                cx: cx.unwrap(),
                cy: cy.unwrap(),
                r: r.unwrap(),
                fx: fx.unwrap(),
                fy: fy.unwrap(),
            },
        }
    }

    fn is_resolved(&self) -> bool {
        match *self {
            UnresolvedVariant::Linear { x1, y1, x2, y2 } => {
                x1.is_some() && y1.is_some() && x2.is_some() && y2.is_some()
            }

            UnresolvedVariant::Radial { cx, cy, r, fx, fy } => {
                cx.is_some() && cy.is_some() && r.is_some() && fx.is_some() && fy.is_some()
            }
        }
    }

    fn resolve_from_fallback(&self, fallback: &UnresolvedVariant) -> UnresolvedVariant {
        match (*self, *fallback) {
            (UnresolvedVariant::Linear { x1, y1, x2, y2 },
             UnresolvedVariant::Linear { x1: fx1, y1: fy1, x2: fx2, y2: fy2 }) => UnresolvedVariant::Linear {
                x1: x1.or(fx1),
                y1: y1.or(fy1),
                x2: x2.or(fx2),
                y2: y2.or(fy2),
            },

            (UnresolvedVariant::Radial { cx, cy, r, fx, fy },
             UnresolvedVariant::Radial { cx: fcx, cy: fcy, r: fr, fx: ffx, fy: ffy }) => UnresolvedVariant::Radial {
                cx: cx.or(fcx),
                cy: cy.or(fcy),
                r: r.or(fr),
                fx: fx.or(ffx),
                fy: fy.or(ffy),
            },

            _ => *self, // If variants are of different types, then nothing to resolve
        }
    }

    // https://www.w3.org/TR/SVG/pservers.html#LinearGradients
    // https://www.w3.org/TR/SVG/pservers.html#RadialGradients
    fn resolve_from_defaults(&self) -> UnresolvedVariant {
        match self {
            UnresolvedVariant::Linear { x1, y1, x2, y2 } => UnresolvedVariant::Linear {
                x1: x1.or_else(|| Some(LengthHorizontal::parse_str("0%").unwrap())),
                y1: y1.or_else(|| Some(LengthVertical::parse_str("0%").unwrap())),
                x2: x2.or_else(|| Some(LengthHorizontal::parse_str("100%").unwrap())),
                y2: y2.or_else(|| Some(LengthVertical::parse_str("0%").unwrap())),
            },

            UnresolvedVariant::Radial { cx, cy, r, fx, fy } => {
                let cx = cx.or_else(|| Some(LengthHorizontal::parse_str("50%").unwrap()));
                let cy = cy.or_else(|| Some(LengthVertical::parse_str("50%").unwrap()));
                let r = r.or_else(|| Some(LengthBoth::parse_str("50%").unwrap()));

                // fx and fy fall back to the presentational value of cx and cy
                let fx = fx.or(cx);
                let fy = fy.or(cy);

                UnresolvedVariant::Radial { cx, cy, r, fx, fy }
            },
        }
    }
}

impl Variant {
    /// Creates a cairo::Gradient corresponding to the gradient type of the
    /// &self Variant.  This does not have color stops set on it yet;
    /// call Gradient.add_color_stops_to_pattern() afterwards.
    fn to_cairo_gradient(&self, values: &ComputedValues, params: &ViewParams) -> cairo::Gradient {
        match *self {
            Variant::Linear { x1, y1, x2, y2 } => {
                cairo::Gradient::clone(&cairo::LinearGradient::new(
                    x1.normalize(values, params),
                    y1.normalize(values, params),
                    x2.normalize(values, params),
                    y2.normalize(values, params),
                ))
            }

            Variant::Radial { cx, cy, r, fx, fy } => {
                let n_cx = cx.normalize(values, params);
                let n_cy = cy.normalize(values, params);
                let n_r = r.normalize(values, params);
                let n_fx = fx.normalize(values, params);
                let n_fy = fy.normalize(values, params);
                let (new_fx, new_fy) = fix_focus_point(n_fx, n_fy, n_cx, n_cy, n_r);

                cairo::Gradient::clone(&cairo::RadialGradient::new(
                    new_fx, new_fy, 0.0, n_cx, n_cy, n_r,
                ))
            }
        }
    }
}

/// Fields shared by all gradient nodes
#[derive(Default)]
struct Common {
    units: Option<GradientUnits>,
    affine: Option<cairo::Matrix>,
    spread: Option<SpreadMethod>,

    fallback: Option<Fragment>,

    resolved: RefCell<Option<Gradient>>,
}

/// Node for the <linearGradient> element
#[derive(Default)]
pub struct NodeLinearGradient {
    common: Common,

    x1: Option<LengthHorizontal>,
    y1: Option<LengthVertical>,
    x2: Option<LengthHorizontal>,
    y2: Option<LengthVertical>,
}

/// Node for the <radialGradient> element
#[derive(Default)]
pub struct NodeRadialGradient {
    common: Common,

    cx: Option<LengthHorizontal>,
    cy: Option<LengthVertical>,
    r: Option<LengthBoth>,
    fx: Option<LengthHorizontal>,
    fy: Option<LengthVertical>,
}

/// Main structure used during gradient resolution.  For unresolved
/// gradients, we store all fields as Option<T> - if None, it means
/// that the field is not specified; if Some(T), it means that the
/// field was specified.
struct UnresolvedGradient {
    units: Option<GradientUnits>,
    affine: Option<cairo::Matrix>,
    spread: Option<SpreadMethod>,
    stops: Option<Vec<ColorStop>>,

    variant: UnresolvedVariant,
}

/// Resolved gradient; this is memoizable after the initial resolution.
#[derive(Clone)]
pub struct Gradient {
    units: GradientUnits,
    affine: cairo::Matrix,
    spread: SpreadMethod,
    stops: Vec<ColorStop>,

    variant: Variant,
}

impl UnresolvedGradient {
    fn to_resolved(self) -> Gradient {
        assert!(self.is_resolved());

        let UnresolvedGradient {
            units,
            affine,
            spread,
            stops,
            variant,
        } = self;

        match variant {
            UnresolvedVariant::Linear { .. } => Gradient {
                units: units.unwrap(),
                affine: affine.unwrap(),
                spread: spread.unwrap(),
                stops: stops.unwrap(),

                variant: variant.to_resolved(),
            },

            UnresolvedVariant::Radial { .. } => Gradient {
                units: units.unwrap(),
                affine: affine.unwrap(),
                spread: spread.unwrap(),
                stops: stops.unwrap(),

                variant: variant.to_resolved(),
            },
        }
    }

    /// Helper for add_color_stops_from_node()
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

    /// Looks for <stop> children inside a linearGradient or radialGradient node,
    /// and adds their info to the UnresolvedGradient &self.
    fn add_color_stops_from_node(&mut self, node: &RsvgNode) {
        let node_type = node.borrow().get_type();

        assert!(node_type == NodeType::LinearGradient || node_type == NodeType::RadialGradient);

        for child_node in node.children() {
            let child = child_node.borrow();

            if child.get_type() != NodeType::Stop {
                continue;
            }

            let stop = child.get_impl::<NodeStop>();

            if child.is_in_error() {
                rsvg_log!("(not using gradient stop {} because it is in error)", child);
            } else {
                let cascaded = CascadedValues::new_from_node(&child_node);
                let values = cascaded.get();
                let rgba = match values.stop_color {
                    StopColor(cssparser::Color::CurrentColor) => values.color.0,
                    StopColor(cssparser::Color::RGBA(ref rgba)) => *rgba,
                };

                self.add_color_stop(stop.offset, rgba, values.stop_opacity.0);
            }
        }
    }

    fn is_resolved(&self) -> bool {
        self.units.is_some()
            && self.affine.is_some()
            && self.spread.is_some()
            && self.stops.is_some()
            && self.variant.is_resolved()
    }

    fn resolve_from_fallback(&self, fallback: &UnresolvedGradient) -> UnresolvedGradient {
        let units = self.units.or(fallback.units);
        let affine = self.affine.or(fallback.affine);
        let spread = self.spread.or(fallback.spread);
        let stops = self.stops.clone().or_else(|| fallback.stops.clone());
        let variant = self.variant.resolve_from_fallback(&fallback.variant);

        UnresolvedGradient { units, affine, spread, stops, variant }
    }

    fn resolve_from_defaults(&self) -> UnresolvedGradient {
        let units = self.units.or(Some(GradientUnits::default()));
        let affine = self.affine.or(Some(cairo::Matrix::identity()));
        let spread = self.spread.or(Some(SpreadMethod::default()));
        let stops = self.stops.clone().or_else(|| Some(Vec::<ColorStop>::new()));
        let variant = self.variant.resolve_from_defaults();

        UnresolvedGradient { units, affine, spread, stops, variant }
    }
}

/// State used during the gradient resolution process
///
/// This is the current node's gradient information, plus the fallback
/// that should be used in case that information is not complete for a
/// resolved gradient yet.
struct Unresolved {
    gradient: UnresolvedGradient,
    fallback: Option<Fragment>,
}

impl NodeLinearGradient {
    fn get_unresolved_variant(&self) -> UnresolvedVariant {
        UnresolvedVariant::Linear {
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
        }
    }
}

impl NodeRadialGradient {
    fn get_unresolved_variant(&self) -> UnresolvedVariant {
        UnresolvedVariant::Radial {
            cx: self.cx,
            cy: self.cy,
            r: self.r,
            fx: self.fx,
            fy: self.fy,
        }
    }
}

macro_rules! impl_get_unresolved {
    ($gradient:ty) => {
        impl $gradient {
            fn get_unresolved(&self, node: &RsvgNode) -> Unresolved {
                let mut gradient = UnresolvedGradient {
                    units: self.common.units,
                    affine: self.common.affine,
                    spread: self.common.spread,
                    stops: None,
                    variant: self.get_unresolved_variant(),
                };

                gradient.add_color_stops_from_node(node);

                Unresolved {
                    gradient,
                    fallback: self.common.fallback.clone(),
                }
            }
        }
    }
}
impl_get_unresolved!(NodeLinearGradient);
impl_get_unresolved!(NodeRadialGradient);

impl Common {
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
}

impl NodeTrait for NodeLinearGradient {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.common.set_atts(pbag)?;

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
}

impl NodeTrait for NodeRadialGradient {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.common.set_atts(pbag)?;

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
}

macro_rules! impl_paint_source {
    ($gradient:ty, $node_type:pat, $other_gradient:ty, $other_type:pat,) => {
        impl PaintSource for $gradient {
            type Resolved = Gradient;

            fn resolve(
                &self,
                node: &RsvgNode,
                draw_ctx: &mut DrawingCtx,
            ) -> Result<Self::Resolved, PaintServerError> {
                let mut resolved = self.common.resolved.borrow_mut();
                if let Some(ref gradient) = *resolved {
                    return Ok(gradient.clone());
                }

                let Unresolved { mut gradient, mut fallback } = self.get_unresolved(node);

                let mut stack = NodeStack::new();

                while !gradient.is_resolved() {
                    if let Some(fragment) = fallback {
                        let acquired = acquire_gradient(draw_ctx, &fragment)?;
                        let acquired_node = acquired.get();

                        if stack.contains(acquired_node) {
                            return Err(PaintServerError::CircularReference(fragment.clone()));
                        }

                        let borrowed_node = acquired_node.borrow();
                        let unresolved = match borrowed_node.get_type() {
                            $node_type => {
                                let a_gradient = borrowed_node.get_impl::<$gradient>();
                                a_gradient.get_unresolved(&acquired_node)
                            }

                            $other_type => {
                                let a_gradient = borrowed_node.get_impl::<$other_gradient>();
                                a_gradient.get_unresolved(&acquired_node)
                            }

                            _ => unreachable!()
                        };

                        gradient = gradient.resolve_from_fallback(&unresolved.gradient);
                        fallback = unresolved.fallback;

                        stack.push(acquired_node);
                    } else {
                        gradient = gradient.resolve_from_defaults();
                        break;
                    }
                }

                let gradient = gradient.to_resolved();

                *resolved = Some(gradient.clone());

                Ok(gradient)
            }
        }
    }
}

impl_paint_source!(
    NodeLinearGradient,
    NodeType::LinearGradient,
    NodeRadialGradient,
    NodeType::RadialGradient,
);

impl_paint_source!(
    NodeRadialGradient,
    NodeType::RadialGradient,
    NodeLinearGradient,
    NodeType::LinearGradient,
);

impl ResolvedPaintSource for Gradient {
    fn set_pattern_on_draw_context(
        self,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: &UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        let params = if self.units == GradientUnits(CoordUnits::ObjectBoundingBox) {
            if bbox.rect.map_or(true, |r| r.is_empty()) {
                // objectBoundingBox requires a non-empty bbox, see issues #187, #373
                return Ok(false);
            }

            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

        let p = match self.variant {
            Variant::Linear { .. } => {
                let g = self.variant.to_cairo_gradient(values, &params);
                cairo::Gradient::clone(&g)
            }

            Variant::Radial { .. } => {
                let g = self.variant.to_cairo_gradient(values, &params);
                cairo::Gradient::clone(&g)
            }
        };

        self.set_on_cairo_pattern(&p, bbox, opacity);

        let cr = draw_ctx.get_cairo_context();
        cr.set_source(&p);

        Ok(true)
    }
}

impl Gradient {
    fn set_on_cairo_pattern(
        &self,
        pattern: &cairo::Gradient,
        bbox: &BoundingBox,
        opacity: &UnitInterval,
    ) {
        let mut affine = self.affine;

        if self.units == GradientUnits(CoordUnits::ObjectBoundingBox) {
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

/// Acquires a node of linearGradient or radialGradient type
fn acquire_gradient<'a>(
    draw_ctx: &'a mut DrawingCtx,
    fragment: &Fragment,
) -> Result<AcquiredNode, PaintServerError> {
    draw_ctx.acquired_nodes().get_node(fragment)
        .ok_or(PaintServerError::LinkNotFound(fragment.clone()))
        .and_then(|acquired| {
            let node_type = acquired.get().borrow().get_type();

            match node_type {
                NodeType::LinearGradient => Ok(acquired),
                NodeType::RadialGradient => Ok(acquired),
                _ => Err(PaintServerError::InvalidLinkType(fragment.clone()))
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
            NodeType::LinearGradient,
            local_name!("linearGradient"),
            None,
            None,
            Box::new(NodeLinearGradient::default())
        ));

        let borrow = node.borrow();
        let g = borrow.get_impl::<NodeLinearGradient>();
        let Unresolved { gradient, .. } = g.get_unresolved(&node);
        let gradient = gradient.resolve_from_defaults();
        assert!(gradient.is_resolved());

        let node = RsvgNode::new(NodeData::new(
            NodeType::RadialGradient,
            local_name!("radialGradient"),
            None,
            None,
            Box::new(NodeRadialGradient::default())
        ));

        let borrow = node.borrow();
        let g = borrow.get_impl::<NodeRadialGradient>();
        let Unresolved { gradient, .. } = g.get_unresolved(&node);
        let gradient = gradient.resolve_from_defaults();
        assert!(gradient.is_resolved());
    }
}
