//! Gradient paint servers; the `linearGradient` and `radialGradient` elements.

use cssparser::Parser;
use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::cell::RefCell;

use crate::allowed_url::Fragment;
use crate::bbox::*;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNode, AcquiredNodes, NodeStack};
use crate::drawing_ctx::{DrawingCtx, ViewParams};
use crate::error::*;
use crate::length::*;
use crate::node::{CascadedValues, NodeResult, NodeTrait, NodeType, RsvgNode};
use crate::paint_server::{AsPaintSource, PaintSource};
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::property_defs::StopColor;
use crate::transform::Transform;
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
    fn parse<'i>(parser: &mut Parser<'i, '_>) -> Result<SpreadMethod, ParseError<'i>> {
        Ok(parse_identifiers!(
            parser,
            "pad" => SpreadMethod::Pad,
            "reflect" => SpreadMethod::Reflect,
            "repeat" => SpreadMethod::Repeat,
        )?)
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
pub struct Stop {
    /// <stop offset="..."/>
    offset: UnitInterval,
    /* stop-color and stop-opacity are not attributes; they are properties, so
     * they go into property_defs.rs */
}

fn validate_offset(length: Length<Both>) -> Result<Length<Both>, ValueErrorKind> {
    match length.unit {
        LengthUnit::Px | LengthUnit::Percent => Ok(length),
        _ => Err(ValueErrorKind::Value(
            "stop offset must be in default or percent units".to_string(),
        )),
    }
}

impl NodeTrait for Stop {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "offset") => {
                    self.offset = attr
                        .parse_and_validate(value, validate_offset)
                        .map(|l| UnitInterval::clamp(l.length))?
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
        x1: Option<Length<Horizontal>>,
        y1: Option<Length<Vertical>>,
        x2: Option<Length<Horizontal>>,
        y2: Option<Length<Vertical>>,
    },

    Radial {
        cx: Option<Length<Horizontal>>,
        cy: Option<Length<Vertical>>,
        r: Option<Length<Both>>,
        fx: Option<Length<Horizontal>>,
        fy: Option<Length<Vertical>>,
    },
}

/// Parameters specific to each gradient type, after resolving.
#[derive(Clone)]
enum Variant {
    Linear {
        x1: Length<Horizontal>,
        y1: Length<Vertical>,
        x2: Length<Horizontal>,
        y2: Length<Vertical>,
    },

    Radial {
        cx: Length<Horizontal>,
        cy: Length<Vertical>,
        r: Length<Both>,
        fx: Length<Horizontal>,
        fy: Length<Vertical>,
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
            (
                UnresolvedVariant::Linear { x1, y1, x2, y2 },
                UnresolvedVariant::Linear {
                    x1: fx1,
                    y1: fy1,
                    x2: fx2,
                    y2: fy2,
                },
            ) => UnresolvedVariant::Linear {
                x1: x1.or(fx1),
                y1: y1.or(fy1),
                x2: x2.or(fx2),
                y2: y2.or(fy2),
            },

            (
                UnresolvedVariant::Radial { cx, cy, r, fx, fy },
                UnresolvedVariant::Radial {
                    cx: fcx,
                    cy: fcy,
                    r: fr,
                    fx: ffx,
                    fy: ffy,
                },
            ) => UnresolvedVariant::Radial {
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
                x1: x1.or_else(|| Some(Length::<Horizontal>::parse_str("0%").unwrap())),
                y1: y1.or_else(|| Some(Length::<Vertical>::parse_str("0%").unwrap())),
                x2: x2.or_else(|| Some(Length::<Horizontal>::parse_str("100%").unwrap())),
                y2: y2.or_else(|| Some(Length::<Vertical>::parse_str("0%").unwrap())),
            },

            UnresolvedVariant::Radial { cx, cy, r, fx, fy } => {
                let cx = cx.or_else(|| Some(Length::<Horizontal>::parse_str("50%").unwrap()));
                let cy = cy.or_else(|| Some(Length::<Vertical>::parse_str("50%").unwrap()));
                let r = r.or_else(|| Some(Length::<Both>::parse_str("50%").unwrap()));

                // fx and fy fall back to the presentational value of cx and cy
                let fx = fx.or(cx);
                let fy = fy.or(cy);

                UnresolvedVariant::Radial { cx, cy, r, fx, fy }
            }
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
    transform: Option<Transform>,
    spread: Option<SpreadMethod>,

    fallback: Option<Fragment>,

    resolved: RefCell<Option<Gradient>>,
}

/// Node for the <linearGradient> element
#[derive(Default)]
pub struct LinearGradient {
    common: Common,

    x1: Option<Length<Horizontal>>,
    y1: Option<Length<Vertical>>,
    x2: Option<Length<Horizontal>>,
    y2: Option<Length<Vertical>>,
}

/// Node for the <radialGradient> element
#[derive(Default)]
pub struct RadialGradient {
    common: Common,

    cx: Option<Length<Horizontal>>,
    cy: Option<Length<Vertical>>,
    r: Option<Length<Both>>,
    fx: Option<Length<Horizontal>>,
    fy: Option<Length<Vertical>>,
}

/// Main structure used during gradient resolution.  For unresolved
/// gradients, we store all fields as Option<T> - if None, it means
/// that the field is not specified; if Some(T), it means that the
/// field was specified.
struct UnresolvedGradient {
    units: Option<GradientUnits>,
    transform: Option<Transform>,
    spread: Option<SpreadMethod>,
    stops: Option<Vec<ColorStop>>,

    variant: UnresolvedVariant,
}

/// Resolved gradient; this is memoizable after the initial resolution.
#[derive(Clone)]
pub struct Gradient {
    units: GradientUnits,
    transform: Transform,
    spread: SpreadMethod,
    stops: Vec<ColorStop>,

    variant: Variant,
}

impl UnresolvedGradient {
    fn to_resolved(self) -> Gradient {
        assert!(self.is_resolved());

        let UnresolvedGradient {
            units,
            transform,
            spread,
            stops,
            variant,
        } = self;

        match variant {
            UnresolvedVariant::Linear { .. } => Gradient {
                units: units.unwrap(),
                transform: transform.unwrap(),
                spread: spread.unwrap(),
                stops: stops.unwrap(),

                variant: variant.to_resolved(),
            },

            UnresolvedVariant::Radial { .. } => Gradient {
                units: units.unwrap(),
                transform: transform.unwrap(),
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

            let stop = child.get_impl::<Stop>();

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
            && self.transform.is_some()
            && self.spread.is_some()
            && self.stops.is_some()
            && self.variant.is_resolved()
    }

    fn resolve_from_fallback(&self, fallback: &UnresolvedGradient) -> UnresolvedGradient {
        let units = self.units.or(fallback.units);
        let transform = self.transform.or(fallback.transform);
        let spread = self.spread.or(fallback.spread);
        let stops = self.stops.clone().or_else(|| fallback.stops.clone());
        let variant = self.variant.resolve_from_fallback(&fallback.variant);

        UnresolvedGradient {
            units,
            transform,
            spread,
            stops,
            variant,
        }
    }

    fn resolve_from_defaults(&self) -> UnresolvedGradient {
        let units = self.units.or_else(|| Some(GradientUnits::default()));
        let transform = self.transform.or_else(|| Some(Transform::default()));
        let spread = self.spread.or_else(|| Some(SpreadMethod::default()));
        let stops = self.stops.clone().or_else(|| Some(Vec::<ColorStop>::new()));
        let variant = self.variant.resolve_from_defaults();

        UnresolvedGradient {
            units,
            transform,
            spread,
            stops,
            variant,
        }
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

impl LinearGradient {
    fn get_unresolved_variant(&self) -> UnresolvedVariant {
        UnresolvedVariant::Linear {
            x1: self.x1,
            y1: self.y1,
            x2: self.x2,
            y2: self.y2,
        }
    }
}

impl RadialGradient {
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
                    transform: self.common.transform,
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
    };
}
impl_get_unresolved!(LinearGradient);
impl_get_unresolved!(RadialGradient);

impl Common {
    fn set_atts(&mut self, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "gradientUnits") => self.units = Some(attr.parse(value)?),
                expanded_name!("", "gradientTransform") => {
                    self.transform = Some(attr.parse(value)?)
                }
                expanded_name!("", "spreadMethod") => self.spread = Some(attr.parse(value)?),
                expanded_name!(xlink "href") => {
                    self.fallback = Some(Fragment::parse(value).attribute(attr)?)
                }
                _ => (),
            }
        }

        Ok(())
    }
}

impl NodeTrait for LinearGradient {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.common.set_atts(pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "x1") => self.x1 = Some(attr.parse(value)?),
                expanded_name!("", "y1") => self.y1 = Some(attr.parse(value)?),
                expanded_name!("", "x2") => self.x2 = Some(attr.parse(value)?),
                expanded_name!("", "y2") => self.y2 = Some(attr.parse(value)?),

                _ => (),
            }
        }

        Ok(())
    }
}

impl NodeTrait for RadialGradient {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        self.common.set_atts(pbag)?;

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "cx") => self.cx = Some(attr.parse(value)?),
                expanded_name!("", "cy") => self.cy = Some(attr.parse(value)?),
                expanded_name!("", "r") => self.r = Some(attr.parse(value)?),
                expanded_name!("", "fx") => self.fx = Some(attr.parse(value)?),
                expanded_name!("", "fy") => self.fy = Some(attr.parse(value)?),

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
                acquired_nodes: &mut AcquiredNodes,
            ) -> Result<Self::Resolved, AcquireError> {
                let mut resolved = self.common.resolved.borrow_mut();
                if let Some(ref gradient) = *resolved {
                    return Ok(gradient.clone());
                }

                let Unresolved {
                    mut gradient,
                    mut fallback,
                } = self.get_unresolved(node);

                let mut stack = NodeStack::new();

                while !gradient.is_resolved() {
                    if let Some(fragment) = fallback {
                        let acquired = acquire_gradient(acquired_nodes, &fragment)?;
                        let acquired_node = acquired.get();

                        if stack.contains(acquired_node) {
                            return Err(AcquireError::CircularReference(acquired_node.clone()));
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

                            _ => unreachable!(),
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
    };
}

impl_paint_source!(
    LinearGradient,
    NodeType::LinearGradient,
    RadialGradient,
    NodeType::RadialGradient,
);

impl_paint_source!(
    RadialGradient,
    NodeType::RadialGradient,
    LinearGradient,
    NodeType::LinearGradient,
);

impl AsPaintSource for Gradient {
    fn set_as_paint_source(
        self,
        _acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        if let Ok(transform) = bbox.rect_to_transform(self.units.0) {
            let params = if self.units == GradientUnits(CoordUnits::ObjectBoundingBox) {
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

            self.set_on_cairo_pattern(&p, &transform, opacity);

            let cr = draw_ctx.get_cairo_context();
            cr.set_source(&p);

            Ok(true)
        } else {
            Ok(false)
        }
    }
}

impl Gradient {
    fn set_on_cairo_pattern(
        &self,
        pattern: &cairo::Gradient,
        transform: &Transform,
        opacity: UnitInterval,
    ) {
        let transform = transform.pre_transform(&self.transform);

        transform.invert().map(|m| pattern.set_matrix(m.into()));
        pattern.set_extend(cairo::Extend::from(self.spread));
        self.add_color_stops_to_pattern(pattern, opacity);
    }

    fn add_color_stops_to_pattern(&self, pattern: &cairo::Gradient, opacity: UnitInterval) {
        for stop in &self.stops {
            let UnitInterval(stop_offset) = stop.offset;
            let UnitInterval(o) = opacity;
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
fn acquire_gradient(
    acquired_nodes: &mut AcquiredNodes,
    fragment: &Fragment,
) -> Result<AcquiredNode, AcquireError> {
    acquired_nodes.acquire(
        fragment,
        &[NodeType::LinearGradient, NodeType::RadialGradient],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::float_eq_cairo::ApproxEqCairo;
    use crate::node::{NodeData, NodeType, RsvgNode};
    use markup5ever::{namespace_url, ns, QualName};

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
            &QualName::new(None, ns!(svg), local_name!("linearGradient")),
            None,
            None,
            Box::new(LinearGradient::default()),
        ));

        let borrow = node.borrow();
        let g = borrow.get_impl::<LinearGradient>();
        let Unresolved { gradient, .. } = g.get_unresolved(&node);
        let gradient = gradient.resolve_from_defaults();
        assert!(gradient.is_resolved());

        let node = RsvgNode::new(NodeData::new(
            NodeType::RadialGradient,
            &QualName::new(None, ns!(svg), local_name!("radialGradient")),
            None,
            None,
            Box::new(RadialGradient::default()),
        ));

        let borrow = node.borrow();
        let g = borrow.get_impl::<RadialGradient>();
        let Unresolved { gradient, .. } = g.get_unresolved(&node);
        let gradient = gradient.resolve_from_defaults();
        assert!(gradient.is_resolved());
    }
}
