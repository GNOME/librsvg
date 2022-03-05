//! Structural elements in SVG: the `g`, `switch`, `svg`, `use`, `symbol`, `clip_path`, `mask`, `link` elements.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::aspect_ratio::*;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNodes, NodeId};
use crate::drawing_ctx::{ClipMode, DrawingCtx, ViewParams};
use crate::element::{Draw, Element, ElementResult, SetAttributes};
use crate::error::*;
use crate::href::{is_href, set_href};
use crate::layout::StackingContext;
use crate::length::*;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::rect::Rect;
use crate::viewbox::*;
use crate::xml::Attributes;

#[derive(Default)]
pub struct Group();

impl SetAttributes for Group {}

impl Draw for Group {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(acquired_nodes, &elt, values.transform(), values);

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |an, dc, _transform| node.draw_children(an, cascaded, dc, clipping),
        )
    }
}

/// A no-op node that does not render anything
///
/// Sometimes we just need a node that can contain children, but doesn't
/// render itself or its children.  This is just that kind of node.
#[derive(Default)]
pub struct NonRendering;

impl SetAttributes for NonRendering {}

impl Draw for NonRendering {}

#[derive(Default)]
pub struct Switch();

impl SetAttributes for Switch {}

impl Draw for Switch {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(acquired_nodes, &elt, values.transform(), values);

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |an, dc, _transform| {
                if let Some(child) = node.children().filter(|c| c.is_element()).find(|c| {
                    let elt = c.borrow_element();
                    elt.get_cond(dc.user_language()) && !elt.is_in_error()
                }) {
                    child.draw(
                        an,
                        &CascadedValues::clone_with_node(cascaded, &child),
                        dc,
                        clipping,
                    )
                } else {
                    Ok(dc.empty_bbox())
                }
            },
        )
    }
}

/// Intrinsic dimensions of an SVG document fragment: its `width/height` properties and  `viewBox` attribute.
///
/// Note that in SVG2, `width` and `height` are properties, not
/// attributes.  If either is omitted, it defaults to `auto`. which
/// computes to `100%`.
///
/// The `viewBox` attribute can also be omitted, hence an `Option`.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct IntrinsicDimensions {
    /// Computed value of the `width` property.
    pub width: ULength<Horizontal>,

    /// Computed value of the `height` property.
    pub height: ULength<Vertical>,

    /// Contents of the `viewBox` attribute.
    pub vbox: Option<ViewBox>,
}

/// The `<svg>` element.
///
/// Note that its x/y/width/height are properties in SVG2, so they are
/// defined as part of [the properties machinery](properties.rs).
#[derive(Default)]
pub struct Svg {
    preserve_aspect_ratio: AspectRatio,
    vbox: Option<ViewBox>,
}

impl Svg {
    pub fn get_intrinsic_dimensions(&self, values: &ComputedValues) -> IntrinsicDimensions {
        let w = match values.width().0 {
            LengthOrAuto::Auto => ULength::<Horizontal>::parse_str("100%").unwrap(),
            LengthOrAuto::Length(l) => l,
        };

        let h = match values.height().0 {
            LengthOrAuto::Auto => ULength::<Vertical>::parse_str("100%").unwrap(),
            LengthOrAuto::Length(l) => l,
        };

        IntrinsicDimensions {
            width: w,
            height: h,
            vbox: self.vbox,
        }
    }

    fn get_unnormalized_offset(
        &self,
        values: &ComputedValues,
    ) -> (Length<Horizontal>, Length<Vertical>) {
        // these defaults are per the spec
        let x = values.x().0;
        let y = values.y().0;

        (x, y)
    }

    fn get_unnormalized_size(
        &self,
        values: &ComputedValues,
    ) -> (ULength<Horizontal>, ULength<Vertical>) {
        // these defaults are per the spec
        let w = match values.width().0 {
            LengthOrAuto::Auto => ULength::<Horizontal>::parse_str("100%").unwrap(),
            LengthOrAuto::Length(l) => l,
        };
        let h = match values.height().0 {
            LengthOrAuto::Auto => ULength::<Vertical>::parse_str("100%").unwrap(),
            LengthOrAuto::Length(l) => l,
        };
        (w, h)
    }

    fn get_viewport(
        &self,
        params: &NormalizeParams,
        values: &ComputedValues,
        outermost: bool,
    ) -> Rect {
        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        let (nx, ny) = if outermost {
            (0.0, 0.0)
        } else {
            let (x, y) = self.get_unnormalized_offset(values);
            (x.to_user(params), y.to_user(params))
        };

        let (w, h) = self.get_unnormalized_size(values);
        let (nw, nh) = (w.to_user(params), h.to_user(params));

        Rect::new(nx, ny, nx + nw, ny + nh)
    }

    fn push_viewport(
        &self,
        node: &Node,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
    ) -> Option<ViewParams> {
        let values = cascaded.get();

        let view_params = draw_ctx.get_view_params();
        let params = NormalizeParams::new(values, &view_params);

        let has_parent = node.parent().is_some();

        let clip_mode = if !values.is_overflow() && has_parent {
            Some(ClipMode::ClipToViewport)
        } else {
            None
        };

        let svg_viewport = self.get_viewport(&params, values, !has_parent);

        let is_measuring_toplevel_svg = !has_parent && draw_ctx.is_measuring();

        let (viewport, vbox) = if is_measuring_toplevel_svg {
            // We are obtaining the toplevel SVG's geometry.  This means, don't care about the
            // DrawingCtx's viewport, just use the SVG's intrinsic dimensions and see how far
            // it wants to extend.
            (svg_viewport, self.vbox)
        } else {
            (
                // The client's viewport overrides the toplevel's x/y/w/h viewport
                if has_parent {
                    svg_viewport
                } else {
                    draw_ctx.toplevel_viewport()
                },
                // Use our viewBox if available, or try to derive one from
                // the intrinsic dimensions.
                self.vbox.or_else(|| {
                    Some(ViewBox::from(Rect::from_size(
                        svg_viewport.width(),
                        svg_viewport.height(),
                    )))
                }),
            )
        };

        draw_ctx.push_new_viewport(vbox, viewport, self.preserve_aspect_ratio, clip_mode)
    }
}

impl SetAttributes for Svg {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => {
                    self.preserve_aspect_ratio = attr.parse(value)?
                }
                expanded_name!("", "viewBox") => self.vbox = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Svg {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(acquired_nodes, &elt, values.transform(), values);

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |an, dc, _transform| {
                let _params = self.push_viewport(node, cascaded, dc);
                node.draw_children(an, cascaded, dc, clipping)
            },
        )
    }
}

pub struct Use {
    link: Option<NodeId>,
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: ULength<Horizontal>,
    height: ULength<Vertical>,
}

impl Use {
    fn get_rect(&self, params: &NormalizeParams) -> Rect {
        let x = self.x.to_user(params);
        let y = self.y.to_user(params);
        let w = self.width.to_user(params);
        let h = self.height.to_user(params);

        Rect::new(x, y, x + w, y + h)
    }
}

impl Default for Use {
    fn default() -> Use {
        Use {
            link: None,
            x: Default::default(),
            y: Default::default(),
            width: ULength::<Horizontal>::parse_str("100%").unwrap(),
            height: ULength::<Vertical>::parse_str("100%").unwrap(),
        }
    }
}

impl SetAttributes for Use {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                ref a if is_href(a) => set_href(
                    a,
                    &mut self.link,
                    NodeId::parse(value).attribute(attr.clone())?,
                ),
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => self.width = attr.parse(value)?,
                expanded_name!("", "height") => self.height = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Use {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        if let Some(link) = self.link.as_ref() {
            let values = cascaded.get();
            let view_params = draw_ctx.get_view_params();
            let params = NormalizeParams::new(values, &view_params);
            let rect = self.get_rect(&params);

            let stroke_paint = values.stroke().0.resolve(
                acquired_nodes,
                values.stroke_opacity().0,
                values.color().0,
                cascaded.context_fill.clone(),
                cascaded.context_stroke.clone(),
            );

            let fill_paint = values.fill().0.resolve(
                acquired_nodes,
                values.fill_opacity().0,
                values.color().0,
                cascaded.context_fill.clone(),
                cascaded.context_stroke.clone(),
            );

            draw_ctx.draw_from_use_node(
                node,
                acquired_nodes,
                values,
                rect,
                link,
                clipping,
                fill_paint,
                stroke_paint,
            )
        } else {
            Ok(draw_ctx.empty_bbox())
        }
    }
}

#[derive(Default)]
pub struct Symbol {
    preserve_aspect_ratio: AspectRatio,
    vbox: Option<ViewBox>,
}

impl Symbol {
    pub fn get_viewbox(&self) -> Option<ViewBox> {
        self.vbox
    }

    pub fn get_preserve_aspect_ratio(&self) -> AspectRatio {
        self.preserve_aspect_ratio
    }
}

impl SetAttributes for Symbol {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => {
                    self.preserve_aspect_ratio = attr.parse(value)?
                }
                expanded_name!("", "viewBox") => self.vbox = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Symbol {}

coord_units!(ClipPathUnits, CoordUnits::UserSpaceOnUse);

#[derive(Default)]
pub struct ClipPath {
    units: ClipPathUnits,
}

impl ClipPath {
    pub fn get_units(&self) -> CoordUnits {
        CoordUnits::from(self.units)
    }
}

impl SetAttributes for ClipPath {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        let result = attrs
            .iter()
            .find(|(attr, _)| attr.expanded() == expanded_name!("", "clipPathUnits"))
            .and_then(|(attr, value)| attr.parse(value).ok());
        if let Some(units) = result {
            self.units = units
        }

        Ok(())
    }
}

impl Draw for ClipPath {}

coord_units!(MaskUnits, CoordUnits::ObjectBoundingBox);
coord_units!(MaskContentUnits, CoordUnits::UserSpaceOnUse);

pub struct Mask {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: ULength<Horizontal>,
    height: ULength<Vertical>,

    units: MaskUnits,
    content_units: MaskContentUnits,
}

impl Default for Mask {
    fn default() -> Mask {
        Mask {
            // these values are per the spec
            x: Length::<Horizontal>::parse_str("-10%").unwrap(),
            y: Length::<Vertical>::parse_str("-10%").unwrap(),
            width: ULength::<Horizontal>::parse_str("120%").unwrap(),
            height: ULength::<Vertical>::parse_str("120%").unwrap(),

            units: MaskUnits::default(),
            content_units: MaskContentUnits::default(),
        }
    }
}

impl Mask {
    pub fn get_units(&self) -> CoordUnits {
        CoordUnits::from(self.units)
    }

    pub fn get_content_units(&self) -> CoordUnits {
        CoordUnits::from(self.content_units)
    }

    pub fn get_rect(&self, params: &NormalizeParams) -> Rect {
        let x = self.x.to_user(params);
        let y = self.y.to_user(params);
        let w = self.width.to_user(params);
        let h = self.height.to_user(params);

        Rect::new(x, y, x + w, y + h)
    }
}

impl SetAttributes for Mask {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => self.width = attr.parse(value)?,
                expanded_name!("", "height") => self.height = attr.parse(value)?,
                expanded_name!("", "maskUnits") => self.units = attr.parse(value)?,
                expanded_name!("", "maskContentUnits") => self.content_units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Mask {}

#[derive(Default)]
pub struct Link {
    pub link: Option<String>,
}

impl SetAttributes for Link {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                ref a if is_href(a) => set_href(a, &mut self.link, value.to_owned()),
                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Link {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        // If this element is inside of <text>, do not draw it.
        // The <text> takes care of it.
        for an in node.ancestors() {
            if matches!(&*an.borrow_element(), Element::Text(_)) {
                return Ok(draw_ctx.empty_bbox());
            }
        }

        let cascaded = CascadedValues::clone_with_node(cascaded, node);
        let values = cascaded.get();

        let elt = node.borrow_element();

        let link_is_empty = self.link.as_ref().map(|l| l.is_empty()).unwrap_or(true);

        let link_target = if link_is_empty {
            None
        } else {
            self.link.clone()
        };

        let stacking_ctx = StackingContext::new_with_link(
            acquired_nodes,
            &elt,
            values.transform(),
            values,
            link_target,
        );

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            values,
            clipping,
            None,
            &mut |an, dc, _transform| node.draw_children(an, &cascaded, dc, clipping),
        )
    }
}
