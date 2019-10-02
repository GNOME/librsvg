use cairo;
use markup5ever::local_name;
use std::cell::RefCell;
use std::f64;

use crate::allowed_url::Fragment;
use crate::aspect_ratio::*;
use crate::bbox::*;
use crate::coord_units::CoordUnits;
use crate::drawing_ctx::{DrawingCtx, NodeStack};
use crate::error::{AttributeResultExt, AcquireError, RenderingError};
use crate::float_eq_cairo::ApproxEqCairo;
use crate::length::*;
use crate::node::*;
use crate::paint_server::{PaintSource, ResolvedPaintSource};
use crate::parsers::ParseValue;
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::rect::RectangleExt;
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
    affine: Option<cairo::Matrix>,
    x: Option<LengthHorizontal>,
    y: Option<LengthVertical>,
    width: Option<LengthHorizontal>,
    height: Option<LengthVertical>,
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

/// Keeps track of which NodePattern provided a non-empty set of children during pattern resolution
#[derive(Clone)]
enum UnresolvedChildren {
    /// Points back to the original NodePattern if it had no usable children
    Unresolved,

    /// Points back to the original NodePattern, as no pattern in the
    /// chain of fallbacks had usable children.  This only gets returned
    /// by resolve_from_defaults().
    ResolvedEmpty,

    /// Points back to the NodePattern that had usable children.
    WithChildren(RsvgWeakNode),
}

/// Keeps track of which NodePattern provided a non-empty set of children during pattern resolution
#[derive(Clone)]
enum Children {
    Empty,

    /// Points back to the NodePattern that had usable children
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

/// Resolved pattern
#[derive(Clone)]
pub struct Pattern {
    units: PatternUnits,
    content_units: PatternContentUnits,
    // This Option<Option<ViewBox>> is a bit strange.  We want a field
    // with value None to mean, "this field isn't resolved yet".  However,
    // the vbox can very well be *not* specified in the SVG file.
    // In that case, the fully resolved pattern will have a .vbox=Some(None) value.
    vbox: Option<ViewBox>,
    preserve_aspect_ratio: AspectRatio,
    affine: cairo::Matrix,
    x: LengthHorizontal,
    y: LengthVertical,
    width: LengthHorizontal,
    height: LengthVertical,

    // Link to the node whose children are the pattern's resolved children.
    children: Children,
}

#[derive(Default)]
pub struct NodePattern {
    common: Common,
    fallback: Option<Fragment>,
    resolved: RefCell<Option<Pattern>>,
}

impl NodeTrait for NodePattern {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("patternUnits") => self.common.units = Some(attr.parse(value)?),
                local_name!("patternContentUnits") => self.common.content_units = Some(attr.parse(value)?),
                local_name!("viewBox") => self.common.vbox = Some(Some(attr.parse(value)?)),
                local_name!("preserveAspectRatio") => {
                    self.common.preserve_aspect_ratio = Some(attr.parse(value)?)
                }
                local_name!("patternTransform") => self.common.affine = Some(attr.parse(value)?),
                local_name!("xlink:href") => {
                    self.fallback = Some(Fragment::parse(value).attribute(attr)?);
                }
                local_name!("x") => self.common.x = Some(attr.parse(value)?),
                local_name!("y") => self.common.y = Some(attr.parse(value)?),
                local_name!("width") => {
                    self.common.width =
                        Some(attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)?)
                }
                local_name!("height") => {
                    self.common.height =
                        Some(attr.parse_and_validate(value, LengthVertical::check_nonnegative)?)
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

impl PaintSource for NodePattern {
    type Resolved = Pattern;

    fn resolve(
        &self,
        node: &RsvgNode,
        draw_ctx: &mut DrawingCtx,
    ) -> Result<Self::Resolved, AcquireError> {
        let mut resolved = self.resolved.borrow_mut();
        if let Some(ref pattern) = *resolved {
            return Ok(pattern.clone());
        }

        let Unresolved { mut pattern, mut fallback } = self.get_unresolved(node);

        let mut stack = NodeStack::new();

        while !pattern.is_resolved() {
            if let Some(ref fragment) = fallback {
                if let Some(acquired) = draw_ctx
                    .acquired_nodes()
                    .get_node_of_type(&fragment, NodeType::Pattern)
                {
                    let acquired_node = acquired.get();

                    if stack.contains(acquired_node) {
                        return Err(AcquireError::CircularReference(fragment.clone()));
                    }

                    let borrowed_node = acquired_node.borrow();
                    let borrowed_pattern = borrowed_node.get_impl::<NodePattern>();
                    let unresolved = borrowed_pattern.get_unresolved(&acquired_node);

                    pattern = pattern.resolve_from_fallback(&unresolved.pattern);
                    fallback = unresolved.fallback;

                    stack.push(acquired_node);
                } else {
                    pattern = pattern.resolve_from_defaults();
                }
            } else {
                pattern = pattern.resolve_from_defaults();
            }
        }

        let pattern = pattern.to_resolved();

        *resolved = Some(pattern.clone());

        Ok(pattern)
    }
}

impl ResolvedPaintSource for Pattern {
    fn set_pattern_on_draw_context(
        self,
        values: &ComputedValues,
        draw_ctx: &mut DrawingCtx,
        _opacity: &UnitInterval,
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

        let (pattern_x, pattern_y, pattern_width, pattern_height) = {
            let params = if units == PatternUnits(CoordUnits::ObjectBoundingBox) {
                draw_ctx.push_view_box(1.0, 1.0)
            } else {
                draw_ctx.get_view_params()
            };

            let pattern_x = self.x.normalize(values, &params);
            let pattern_y = self.y.normalize(values, &params);
            let pattern_width = self.width.normalize(values, &params);
            let pattern_height = self.height.normalize(values, &params);

            (pattern_x, pattern_y, pattern_width, pattern_height)
        };

        // Work out the size of the rectangle so it takes into account the object bounding box

        let bbwscale: f64;
        let bbhscale: f64;

        match units {
            PatternUnits(CoordUnits::ObjectBoundingBox) => {
                let bbrect = bbox.rect.unwrap();
                bbwscale = bbrect.width;
                bbhscale = bbrect.height;
            }

            PatternUnits(CoordUnits::UserSpaceOnUse) => {
                bbwscale = 1.0;
                bbhscale = 1.0;
            }
        }

        let cr = draw_ctx.get_cairo_context();
        let affine = cr.get_matrix();
        let taffine = cairo::Matrix::multiply(&pattern_affine, &affine);

        let mut scwscale = (taffine.xx * taffine.xx + taffine.xy * taffine.xy).sqrt();
        let mut schscale = (taffine.yx * taffine.yx + taffine.yy * taffine.yy).sqrt();

        let pw: i32 = (pattern_width * bbwscale * scwscale) as i32;
        let ph: i32 = (pattern_height * bbhscale * schscale) as i32;

        let scaled_width = pattern_width * bbwscale;
        let scaled_height = pattern_height * bbhscale;

        if scaled_width.abs() < f64::EPSILON
            || scaled_height.abs() < f64::EPSILON
            || pw < 1
            || ph < 1
        {
            return Ok(false);
        }

        scwscale = f64::from(pw) / scaled_width;
        schscale = f64::from(ph) / scaled_height;

        let mut affine: cairo::Matrix = cairo::Matrix::identity();

        // Create the pattern coordinate system
        match units {
            PatternUnits(CoordUnits::ObjectBoundingBox) => {
                let bbrect = bbox.rect.unwrap();
                affine.translate(
                    bbrect.x + pattern_x * bbrect.width,
                    bbrect.y + pattern_y * bbrect.height,
                );
            }

            PatternUnits(CoordUnits::UserSpaceOnUse) => {
                affine.translate(pattern_x, pattern_y);
            }
        }

        // Apply the pattern transform
        affine = cairo::Matrix::multiply(&affine, &pattern_affine);

        let mut caffine: cairo::Matrix;

        // Create the pattern contents coordinate system
        let _params = if let Some(vbox) = vbox {
            // If there is a vbox, use that
            let (mut x, mut y, w, h) = preserve_aspect_ratio.compute(
                &vbox,
                &cairo::Rectangle::new(
                    0.0,
                    0.0,
                    pattern_width * bbwscale,
                    pattern_height * bbhscale,
                ),
            );

            x -= vbox.x * w / vbox.width;
            y -= vbox.y * h / vbox.height;

            caffine = cairo::Matrix::new(w / vbox.width, 0.0, 0.0, h / vbox.height, x, y);

            draw_ctx.push_view_box(vbox.width, vbox.height)
        } else if content_units == PatternContentUnits(CoordUnits::ObjectBoundingBox) {
            // If coords are in terms of the bounding box, use them
            let bbrect = bbox.rect.unwrap();

            caffine = cairo::Matrix::identity();
            caffine.scale(bbrect.width, bbrect.height);

            draw_ctx.push_view_box(1.0, 1.0)
        } else {
            caffine = cairo::Matrix::identity();
            draw_ctx.get_view_params()
        };

        if !scwscale.approx_eq_cairo(1.0) || !schscale.approx_eq_cairo(1.0) {
            let mut scalematrix = cairo::Matrix::identity();
            scalematrix.scale(scwscale, schscale);
            caffine = cairo::Matrix::multiply(&caffine, &scalematrix);

            scalematrix = cairo::Matrix::identity();
            scalematrix.scale(1.0 / scwscale, 1.0 / schscale);

            affine = cairo::Matrix::multiply(&scalematrix, &affine);
        }

        // Draw to another surface

        let cr_save = draw_ctx.get_cairo_context();

        let surface = cr_save
            .get_target()
            .create_similar(cairo::Content::ColorAlpha, pw, ph);

        let cr_pattern = cairo::Context::new(&surface);

        draw_ctx.set_cairo_context(&cr_pattern);

        // Set up transformations to be determined by the contents units

        // Draw everything
        let pattern_cascaded = CascadedValues::new_from_node(&node_with_children);
        let pattern_values = pattern_cascaded.get();

        cr_pattern.set_matrix(caffine);

        let res = draw_ctx.with_discrete_layer(&node_with_children, pattern_values, false, &mut |dc| {
            node_with_children.draw_children(&pattern_cascaded, dc, false)
        });

        // Return to the original coordinate system and rendering context

        draw_ctx.set_cairo_context(&cr_save);

        // Set the final surface as a Cairo pattern into the Cairo context

        let surface_pattern = cairo::SurfacePattern::create(&surface);
        surface_pattern.set_extend(cairo::Extend::Repeat);

        let mut matrix = affine;
        matrix.invert();

        surface_pattern.set_matrix(matrix);
        surface_pattern.set_filter(cairo::Filter::Best);

        cr_save.set_source(&surface_pattern);

        res.and_then(|_| Ok(true))
    }
}

impl UnresolvedPattern {
    fn to_resolved(self) -> Pattern {
        assert!(self.is_resolved());

        Pattern {
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
        let preserve_aspect_ratio = self.common.preserve_aspect_ratio.or(fallback.common.preserve_aspect_ratio);
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
        let units = self.common.units.or(Some(PatternUnits::default()));
        let content_units = self.common.content_units.or(Some(PatternContentUnits::default()));
        let vbox = self.common.vbox.or(Some(None));
        let preserve_aspect_ratio = self.common.preserve_aspect_ratio.or(Some(AspectRatio::default()));
        let affine = self.common.affine.or(Some(cairo::Matrix::identity()));
        let x = self.common.x.or(Some(Default::default()));
        let y = self.common.y.or(Some(Default::default()));
        let width = self.common.width.or(Some(Default::default()));
        let height = self.common.height.or(Some(Default::default()));
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

        if node.children().any(|child| child.borrow().get_type() != NodeType::Chars) {
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

impl NodePattern {
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

    #[test]
    fn pattern_resolved_from_defaults_is_really_resolved() {
        let node = RsvgNode::new(NodeData::new(
            NodeType::Pattern,
            local_name!("pattern"),
            None,
            None,
            Box::new(NodePattern::default())
        ));

        let borrow = node.borrow();
        let p = borrow.get_impl::<NodePattern>();
        let Unresolved { pattern, .. } = p.get_unresolved(&node);
        let pattern = pattern.resolve_from_defaults();
        assert!(pattern.is_resolved());
    }
}
