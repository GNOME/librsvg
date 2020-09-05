//! The `pattern` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};
use std::cell::RefCell;

use crate::allowed_url::Fragment;
use crate::aspect_ratio::*;
use crate::attributes::Attributes;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNodes, NodeStack};
use crate::drawing_ctx::ViewParams;
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::error::*;
use crate::href::{is_href, set_href};
use crate::length::*;
use crate::node::{Node, NodeBorrow, WeakNode};
use crate::parsers::ParseValue;
use crate::properties::ComputedValues;
use crate::rect::Rect;
use crate::transform::Transform;
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
    WithChildren(WeakNode),
}

/// Keeps track of which Pattern provided a non-empty set of children during pattern resolution
#[derive(Clone)]
enum Children {
    Empty,

    /// Points back to the Pattern that had usable children
    WithChildren(WeakNode),
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

impl SetAttributes for Pattern {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
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
                ref a if is_href(a) => {
                    set_href(
                        a,
                        &mut self.fallback,
                        Fragment::parse(value).attribute(attr.clone())?,
                    );
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
}

impl Draw for Pattern {}

impl UnresolvedPattern {
    fn into_resolved(self) -> ResolvedPattern {
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
    fn from_node(node: &Node) -> UnresolvedChildren {
        let weak = node.downgrade();

        if node.children().any(|child| child.is_element()) {
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

impl ResolvedPattern {
    pub fn get_units(&self) -> PatternUnits {
        self.units
    }

    pub fn get_content_units(&self) -> PatternContentUnits {
        self.content_units
    }

    pub fn get_vbox(&self) -> Option<ViewBox> {
        self.vbox
    }

    pub fn get_preserve_aspect_ratio(&self) -> AspectRatio {
        self.preserve_aspect_ratio
    }

    pub fn get_transform(&self) -> Transform {
        self.affine
    }

    pub fn get_rect(&self, values: &ComputedValues, params: &ViewParams) -> Rect {
        let x = self.x.normalize(&values, &params);
        let y = self.y.normalize(&values, &params);
        let w = self.width.normalize(&values, &params);
        let h = self.height.normalize(&values, &params);

        Rect::new(x, y, x + w, y + h)
    }

    pub fn node_with_children(&self) -> Option<Node> {
        match self.children {
            Children::Empty => None,
            Children::WithChildren(ref wc) => Some(wc.upgrade().unwrap()),
        }
    }
}

impl Pattern {
    fn get_unresolved(&self, node: &Node) -> Unresolved {
        let pattern = UnresolvedPattern {
            common: self.common.clone(),
            children: UnresolvedChildren::from_node(node),
        };

        Unresolved {
            pattern,
            fallback: self.fallback.clone(),
        }
    }

    pub fn resolve(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
    ) -> Result<ResolvedPattern, AcquireError> {
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
                match acquired_nodes.acquire(&fragment) {
                    Ok(acquired) => {
                        let acquired_node = acquired.get();

                        if stack.contains(acquired_node) {
                            return Err(AcquireError::CircularReference(acquired_node.clone()));
                        }

                        match *acquired_node.borrow_element() {
                            Element::Pattern(ref p) => {
                                let unresolved = p.get_unresolved(&acquired_node);
                                pattern = pattern.resolve_from_fallback(&unresolved.pattern);
                                fallback = unresolved.fallback;

                                stack.push(acquired_node);
                            }
                            _ => return Err(AcquireError::InvalidLinkType(fragment.clone())),
                        }
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

        let pattern = pattern.into_resolved();

        *resolved = Some(pattern.clone());

        Ok(pattern)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::NodeData;
    use markup5ever::{namespace_url, ns, QualName};
    use std::ptr;

    #[test]
    fn pattern_resolved_from_defaults_is_really_resolved() {
        let attrs = unsafe { Attributes::new_from_xml2_attributes(0, ptr::null()) };

        let node = Node::new(NodeData::new_element(
            &QualName::new(None, ns!(svg), local_name!("pattern")),
            attrs,
        ));

        let unresolved = borrow_element_as!(node, Pattern).get_unresolved(&node);
        let pattern = unresolved.pattern.resolve_from_defaults();
        assert!(pattern.is_resolved());
    }
}
