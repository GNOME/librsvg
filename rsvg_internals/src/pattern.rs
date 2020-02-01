//! The `pattern` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::cell::RefCell;
use std::f64;

use crate::allowed_url::Fragment;
use crate::aspect_ratio::*;
use crate::bbox::*;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNodes, NodeStack};
use crate::drawing_ctx::{DrawingCtx, ViewParams};
use crate::error::*;
use crate::float_eq_cairo::ApproxEqCairo;
use crate::length::*;
use crate::node::*;
use crate::paint_server::{AsPaintSource, PaintSource};
use crate::parsers::ParseValue;
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::rect::Rect;
use crate::transform::Transform;
use crate::unit_interval::UnitInterval;
use crate::viewbox::*;

coord_units!(PatternUnits, CoordUnits::ObjectBoundingBox);
coord_units!(PatternContentUnits, CoordUnits::UserSpaceOnUse);

#[derive(Clone, Default)]
struct Common {
    units: Option<PatternUnits>,
    content_units: Option<PatternContentUnits>,
    // This Option<Option<ViewBox>> is a bit strange.  We want a field
    // with value None to mean, "this field isn't resolved yet".  However,
    // the vbox can very well be *not* specified in the SVG file.
    // In that case, the fully resolved pattern will have a .vbox=Some(None) value.
    vbox: Option<Option<ViewBox>>,
    preserve_aspect_ratio: Option<AspectRatio>,
    affine: Option<Transform>,
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    width: Option<Length<Horizontal>>,
    height: Option<Length<Vertical>>,
}

/// State used during the pattern resolution process
///
/// This is the current node's pattern information, plus the fallback
/// that should be used in case that information is not complete for a
/// resolved pattern yet.
struct Unresolved {
    pattern: UnresolvedPattern,
    fallback: Option<Fragment>,
}

/// Keeps track of which Pattern provided a non-empty set of children during pattern resolution
#[derive(Clone)]
enum UnresolvedChildren {
    /// Points back to the original Pattern if it had no usable children
    Unresolved,

    /// Points back to the original Pattern, as no pattern in the
    /// chain of fallbacks had usable children.  This only gets returned
    /// by resolve_from_defaults().
    ResolvedEmpty,

    /// Points back to the Pattern that had usable children.
    WithChildren(RsvgWeakNode),
}

/// Keeps track of which Pattern provided a non-empty set of children during pattern resolution
#[derive(Clone)]
enum Children {
    Empty,

    /// Points back to the Pattern that had usable children
    WithChildren(RsvgWeakNode),
}

/// Main structure used during pattern resolution.  For unresolved
/// patterns, we store all fields as Option<T> - if None, it means
/// that the field is not specified; if Some(T), it means that the
/// field was specified.
struct UnresolvedPattern {
    common: Common,

    // Point back to our corresponding node, or to the fallback node which has children.
    // If the value is None, it means we are fully resolved and didn't find any children
    // among the fallbacks.
    children: UnresolvedChildren,
}

#[derive(Clone)]
pub struct ResolvedPattern {
    units: PatternUnits,
    content_units: PatternContentUnits,
    // This Option<Option<ViewBox>> is a bit strange.  We want a field
    // with value None to mean, "this field isn't resolved yet".  However,
    // the vbox can very well be *not* specified in the SVG file.
    // In that case, the fully resolved pattern will have a .vbox=Some(None) value.
    vbox: Option<ViewBox>,
    preserve_aspect_ratio: AspectRatio,
    affine: Transform,
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: Length<Horizontal>,
    height: Length<Vertical>,

    // Link to the node whose children are the pattern's resolved children.
    children: Children,
}

#[derive(Default)]
pub struct Pattern {
    common: Common,
    fallback: Option<Fragment>,
    resolved: RefCell<Option<ResolvedPattern>>,
}

impl NodeTrait for Pattern {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!("", "patternUnits") => self.common.units = Some(attr.parse(value)?),
                expanded_name!("", "patternContentUnits") => {
                    self.common.content_units = Some(attr.parse(value)?)
                }
                expanded_name!("", "viewBox") => self.common.vbox = Some(Some(attr.parse(value)?)),
                expanded_name!("", "preserveAspectRatio") => {
                    self.common.preserve_aspect_ratio = Some(attr.parse(value)?)
                }
                expanded_name!("", "patternTransform") => {
                    self.common.affine = Some(attr.parse(value)?)
                }
                expanded_name!(xlink "href") => {
                    self.fallback = Some(Fragment::parse(value).attribute(attr)?);
                }
                expanded_name!("", "x") => self.common.x = Some(attr.parse(value)?),
                expanded_name!("", "y") => self.common.y = Some(attr.parse(value)?),
                expanded_name!("", "width") => {
                    self.common.width = Some(
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?,
                    )
                }
                expanded_name!("", "height") => {
                    self.common.height =
                        Some(attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?)
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn overflow_hidden(&self) -> bool {
        true
    }
}

impl PaintSource for Pattern {
    type Resolved = ResolvedPattern;

    fn resolve(
        &self,
        node: &RsvgNode,
        acquired_nodes: &mut AcquiredNodes,
    ) -> Result<Self::Resolved, AcquireError> {
        let mut resolved = self.resolved.borrow_mut();
        if let Some(ref pattern) = *resolved {
            return Ok(pattern.clone());
        }

        let Unresolved {
            mut pattern,
            mut fallback,
        } = self.get_unresolved(node);

        let mut stack = NodeStack::new();

        while !pattern.is_resolved() {
            if let Some(ref fragment) = fallback {
                match acquired_nodes.acquire(&fragment, &[NodeType::Pattern]) {
                    Ok(acquired) => {
                        let acquired_node = acquired.get();

                        if stack.contains(acquired_node) {
                            return Err(AcquireError::CircularReference(acquired_node.clone()));
                        }

                        let borrowed_node = acquired_node.borrow();
                        let borrowed_pattern = borrowed_node.get_impl::<Pattern>();
                        let unresolved = borrowed_pattern.get_unresolved(&acquired_node);

                        pattern = pattern.resolve_from_fallback(&unresolved.pattern);
                        fallback = unresolved.fallback;

                        stack.push(acquired_node);
                    }

                    Err(AcquireError::MaxReferencesExceeded) => {
                        return Err(AcquireError::MaxReferencesExceeded)
                    }

                    Err(e) => {
                        rsvg_log!("Stopping pattern resolution: {}", e);
                        pattern = pattern.resolve_from_defaults();
                        break;
                    }
                }
            } else {
                pattern = pattern.resolve_from_defaults();
                break;
            }
        }

        let pattern = pattern.to_resolved();

        *resolved = Some(pattern.clone());

        Ok(pattern)
    }
}

impl AsPaintSource for ResolvedPattern {
    fn set_as_paint_source(
        self,
        acquired_nodes: &mut AcquiredNodes,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        opacity: UnitInterval,
        bbox: &BoundingBox,
    ) -> Result<bool, RenderingError> {
        let node_with_children = if let Some(n) = self.children.node_with_children() {
            n
        } else {
            // This means we didn't find any children among the fallbacks,
            // so there is nothing to render.
            return Ok(false);
        };

        let units = self.units;
        let content_units = self.content_units;
        let pattern_affine = self.affine;
        let vbox = self.vbox;
        let preserve_aspect_ratio = self.preserve_aspect_ratio;

        let params = if units == PatternUnits(CoordUnits::ObjectBoundingBox) {
            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            draw_ctx.get_view_params()
        };

        let pattern_rect = self.get_rect(values, &params);

        // Work out the size of the rectangle so it takes into account the object bounding box

        let (bbwscale, bbhscale) = match units {
            PatternUnits(CoordUnits::ObjectBoundingBox) => bbox.rect.unwrap().size(),
            PatternUnits(CoordUnits::UserSpaceOnUse) => (1.0, 1.0),
        };

        let taffine = draw_ctx.get_transform().pre_transform(&pattern_affine);

        let mut scwscale = (taffine.xx.powi(2) + taffine.xy.powi(2)).sqrt();
        let mut schscale = (taffine.yx.powi(2) + taffine.yy.powi(2)).sqrt();

        let scaled_width = pattern_rect.width() * bbwscale;
        let scaled_height = pattern_rect.height() * bbhscale;

        let pw: i32 = (scaled_width * scwscale) as i32;
        let ph: i32 = (scaled_height * schscale) as i32;

        if scaled_width.abs() < f64::EPSILON
            || scaled_height.abs() < f64::EPSILON
            || pw < 1
            || ph < 1
        {
            return Ok(false);
        }

        scwscale = f64::from(pw) / scaled_width;
        schscale = f64::from(ph) / scaled_height;

        // Create the pattern coordinate system
        let mut affine = match units {
            PatternUnits(CoordUnits::ObjectBoundingBox) => {
                let bbrect = bbox.rect.unwrap();
                Transform::new_translate(
                    bbrect.x0 + pattern_rect.x0 * bbrect.width(),
                    bbrect.y0 + pattern_rect.y0 * bbrect.height(),
                )
            }

            PatternUnits(CoordUnits::UserSpaceOnUse) => {
                Transform::new_translate(pattern_rect.x0, pattern_rect.y0)
            }
        };

        // Apply the pattern transform
        affine = affine.post_transform(&pattern_affine);

        let mut caffine: Transform;

        // Create the pattern contents coordinate system
        let _params = if let Some(vbox) = vbox {
            // If there is a vbox, use that
            let r =
                preserve_aspect_ratio.compute(&vbox, &Rect::from_size(scaled_width, scaled_height));

            let sw = r.width() / vbox.0.width();
            let sh = r.height() / vbox.0.height();
            let x = r.x0 - vbox.0.x0 * sw;
            let y = r.y0 - vbox.0.y0 * sh;

            caffine = Transform::new_scale(sw, sh).pre_translate(x, y);

            draw_ctx.push_view_box(vbox.0.width(), vbox.0.height())
        } else if content_units == PatternContentUnits(CoordUnits::ObjectBoundingBox) {
            // If coords are in terms of the bounding box, use them
            let (bbw, bbh) = bbox.rect.unwrap().size();

            caffine = Transform::new_scale(bbw, bbh);

            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            caffine = Transform::identity();
            draw_ctx.get_view_params()
        };

        if !scwscale.approx_eq_cairo(1.0) || !schscale.approx_eq_cairo(1.0) {
            caffine = caffine.post_scale(scwscale, schscale);
            affine = affine.pre_scale(1.0 / scwscale, 1.0 / schscale);
        }

        // Draw to another surface

        let cr_save = draw_ctx.get_cairo_context();

        let surface = cr_save
            .get_target()
            .create_similar(cairo::Content::ColorAlpha, pw, ph)?;

        let cr_pattern = cairo::Context::new(&surface);

        draw_ctx.set_cairo_context(&cr_pattern);

        // Set up transformations to be determined by the contents units

        // Draw everything
        let pattern_cascaded = CascadedValues::new_from_node(&node_with_children);
        let pattern_values = pattern_cascaded.get();

        cr_pattern.set_matrix(caffine.into());

        let UnitInterval(o) = opacity;
        if o < 1.0 {
            cr_pattern.push_group();
        }

        let res = draw_ctx.with_discrete_layer(
            &node_with_children,
            acquired_nodes,
            pattern_values,
            false,
            &mut |an, dc| node_with_children.draw_children(an, &pattern_cascaded, dc, false),
        );

        if o < 1.0 {
            cr_pattern.pop_group_to_source();
            cr_pattern.paint_with_alpha(o);
        }

        // Return to the original coordinate system and rendering context
        draw_ctx.set_cairo_context(&cr_save);

        // Set the final surface as a Cairo pattern into the Cairo context
        let pattern = cairo::SurfacePattern::create(&surface);

        affine.invert().map(|m| pattern.set_matrix(m.into()));
        pattern.set_extend(cairo::Extend::Repeat);
        pattern.set_filter(cairo::Filter::Best);
        cr_save.set_source(&pattern);

        res.and_then(|_| Ok(true))
    }
}

impl UnresolvedPattern {
    fn to_resolved(self) -> ResolvedPattern {
        assert!(self.is_resolved());

        ResolvedPattern {
            units: self.common.units.unwrap(),
            content_units: self.common.content_units.unwrap(),
            vbox: self.common.vbox.unwrap(),
            preserve_aspect_ratio: self.common.preserve_aspect_ratio.unwrap(),
            affine: self.common.affine.unwrap(),
            x: self.common.x.unwrap(),
            y: self.common.y.unwrap(),
            width: self.common.width.unwrap(),
            height: self.common.height.unwrap(),

            children: self.children.to_resolved(),
        }
    }

    fn is_resolved(&self) -> bool {
        self.common.units.is_some()
            && self.common.content_units.is_some()
            && self.common.vbox.is_some()
            && self.common.preserve_aspect_ratio.is_some()
            && self.common.affine.is_some()
            && self.common.x.is_some()
            && self.common.y.is_some()
            && self.common.width.is_some()
            && self.common.height.is_some()
            && self.children.is_resolved()
    }

    fn resolve_from_fallback(&self, fallback: &UnresolvedPattern) -> UnresolvedPattern {
        let units = self.common.units.or(fallback.common.units);
        let content_units = self.common.content_units.or(fallback.common.content_units);
        let vbox = self.common.vbox.or(fallback.common.vbox);
        let preserve_aspect_ratio = self
            .common
            .preserve_aspect_ratio
            .or(fallback.common.preserve_aspect_ratio);
        let affine = self.common.affine.or(fallback.common.affine);
        let x = self.common.x.or(fallback.common.x);
        let y = self.common.y.or(fallback.common.y);
        let width = self.common.width.or(fallback.common.width);
        let height = self.common.height.or(fallback.common.height);
        let children = self.children.resolve_from_fallback(&fallback.children);

        UnresolvedPattern {
            common: Common {
                units,
                content_units,
                vbox,
                preserve_aspect_ratio,
                affine,
                x,
                y,
                width,
                height,
            },
            children,
        }
    }

    fn resolve_from_defaults(&self) -> UnresolvedPattern {
        let units = self.common.units.or_else(|| Some(PatternUnits::default()));
        let content_units = self
            .common
            .content_units
            .or_else(|| Some(PatternContentUnits::default()));
        let vbox = self.common.vbox.or(Some(None));
        let preserve_aspect_ratio = self
            .common
            .preserve_aspect_ratio
            .or_else(|| Some(AspectRatio::default()));
        let affine = self.common.affine.or_else(|| Some(Transform::default()));
        let x = self.common.x.or_else(|| Some(Default::default()));
        let y = self.common.y.or_else(|| Some(Default::default()));
        let width = self.common.width.or_else(|| Some(Default::default()));
        let height = self.common.height.or_else(|| Some(Default::default()));
        let children = self.children.resolve_from_defaults();

        UnresolvedPattern {
            common: Common {
                units,
                content_units,
                vbox,
                preserve_aspect_ratio,
                affine,
                x,
                y,
                width,
                height,
            },
            children,
        }
    }
}

impl UnresolvedChildren {
    fn from_node(node: &RsvgNode) -> UnresolvedChildren {
        let weak = node.downgrade();

        if node
            .children()
            .any(|child| child.borrow().get_type() != NodeType::Chars)
        {
            UnresolvedChildren::WithChildren(weak)
        } else {
            UnresolvedChildren::Unresolved
        }
    }

    fn is_resolved(&self) -> bool {
        match *self {
            UnresolvedChildren::Unresolved => false,
            _ => true,
        }
    }

    fn resolve_from_fallback(&self, fallback: &UnresolvedChildren) -> UnresolvedChildren {
        use UnresolvedChildren::*;

        match (self, fallback) {
            (&Unresolved, &Unresolved) => Unresolved,
            (&WithChildren(ref wc), _) => WithChildren(wc.clone()),
            (_, &WithChildren(ref wc)) => WithChildren(wc.clone()),
            (_, _) => unreachable!(),
        }
    }

    fn resolve_from_defaults(&self) -> UnresolvedChildren {
        use UnresolvedChildren::*;

        match *self {
            Unresolved => ResolvedEmpty,
            _ => (*self).clone(),
        }
    }

    fn to_resolved(&self) -> Children {
        use UnresolvedChildren::*;

        assert!(self.is_resolved());

        match *self {
            ResolvedEmpty => Children::Empty,
            WithChildren(ref wc) => Children::WithChildren(wc.clone()),
            _ => unreachable!(),
        }
    }
}

impl Children {
    fn node_with_children(&self) -> Option<RsvgNode> {
        match *self {
            Children::Empty => None,
            Children::WithChildren(ref wc) => Some(wc.upgrade().unwrap()),
        }
    }
}

impl ResolvedPattern {
    fn get_rect(&self, values: &ComputedValues, params: &ViewParams) -> Rect {
        let x = self.x.normalize(&values, &params);
        let y = self.y.normalize(&values, &params);
        let w = self.width.normalize(&values, &params);
        let h = self.height.normalize(&values, &params);

        Rect::new(x, y, x + w, y + h)
    }
}

impl Pattern {
    fn get_unresolved(&self, node: &RsvgNode) -> Unresolved {
        let pattern = UnresolvedPattern {
            common: self.common.clone(),
            children: UnresolvedChildren::from_node(node),
        };

        Unresolved {
            pattern,
            fallback: self.fallback.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{NodeData, NodeType, RsvgNode};
    use markup5ever::{namespace_url, ns, QualName};

    #[test]
    fn pattern_resolved_from_defaults_is_really_resolved() {
        let node = RsvgNode::new(NodeData::new(
            NodeType::Pattern,
            &QualName::new(None, ns!(svg), local_name!("pattern")),
            None,
            None,
            Box::new(Pattern::default()),
        ));

        let borrow = node.borrow();
        let p = borrow.get_impl::<Pattern>();
        let Unresolved { pattern, .. } = p.get_unresolved(&node);
        let pattern = pattern.resolve_from_defaults();
        assert!(pattern.is_resolved());
    }
}
