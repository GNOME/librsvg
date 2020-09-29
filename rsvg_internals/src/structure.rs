//! Structural elements in SVG: the `g`, `switch`, `svg`, `use`, `symbol`, `clip_path`, `mask`, `link` elements.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::allowed_url::Fragment;
use crate::aspect_ratio::*;
use crate::attributes::Attributes;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::{ClipMode, DrawingCtx, ViewParams};
use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::*;
use crate::href::{is_href, set_href};
use crate::length::*;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::rect::Rect;
use crate::viewbox::*;

#[derive(Default)]
pub struct Group();

impl SetAttributes for Group {}

impl Draw for Group {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
            node.draw_children(an, cascaded, dc, clipping)
        })
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
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
            if let Some(child) = node
                .children()
                .filter(|c| c.is_element())
                .find(|c| c.borrow_element().get_cond())
            {
                dc.draw_node_from_stack(
                    &child,
                    an,
                    &CascadedValues::new(cascaded, &child),
                    clipping,
                )
            } else {
                Ok(dc.empty_bbox())
            }
        })
    }
}

/// Intrinsic dimensions of an SVG document fragment: its `width`, `height`, `viewBox` attributes.
///
/// Note that either of those attributes can be omitted, so they are all `Option<T>`.
/// For example, an element like `<svg viewBox="0 0 100 100">` will have `vbox=Some(...)`,
/// and the other two fields set to `None`.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct IntrinsicDimensions {
    /// Contents of the `width` attribute.
    pub width: Option<Length<Horizontal>>,

    /// Contents of the `height` attribute.
    pub height: Option<Length<Vertical>>,

    /// Contents of the `viewBox` attribute.
    pub vbox: Option<ViewBox>,
}

#[derive(Default)]
pub struct Svg {
    preserve_aspect_ratio: AspectRatio,
    x: Option<Length<Horizontal>>,
    y: Option<Length<Vertical>>,
    w: Option<Length<Horizontal>>,
    h: Option<Length<Vertical>>,
    vbox: Option<ViewBox>,
}

impl Svg {
    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        IntrinsicDimensions {
            width: self.w,
            height: self.h,
            vbox: self.vbox,
        }
    }

    fn get_unnormalized_offset(&self) -> (Length<Horizontal>, Length<Vertical>) {
        // these defaults are per the spec
        let x = self
            .x
            .unwrap_or_else(|| Length::<Horizontal>::parse_str("0").unwrap());
        let y = self
            .y
            .unwrap_or_else(|| Length::<Vertical>::parse_str("0").unwrap());

        (x, y)
    }

    fn get_unnormalized_size(&self) -> (Length<Horizontal>, Length<Vertical>) {
        // these defaults are per the spec
        let w = self
            .w
            .unwrap_or_else(|| Length::<Horizontal>::parse_str("100%").unwrap());
        let h = self
            .h
            .unwrap_or_else(|| Length::<Vertical>::parse_str("100%").unwrap());

        (w, h)
    }

    fn get_viewport(&self, values: &ComputedValues, params: &ViewParams, outermost: bool) -> Rect {
        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        let (nx, ny) = if outermost {
            (0.0, 0.0)
        } else {
            let (x, y) = self.get_unnormalized_offset();
            (x.normalize(values, &params), y.normalize(values, &params))
        };

        let (w, h) = self.get_unnormalized_size();
        let (nw, nh) = (w.normalize(values, &params), h.normalize(values, &params));

        Rect::new(nx, ny, nx + nw, ny + nh)
    }
}

impl SetAttributes for Svg {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => {
                    self.preserve_aspect_ratio = attr.parse(value)?
                }
                expanded_name!("", "x") => self.x = Some(attr.parse(value)?),
                expanded_name!("", "y") => self.y = Some(attr.parse(value)?),
                expanded_name!("", "width") => {
                    self.w = Some(
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?,
                    )
                }
                expanded_name!("", "height") => {
                    self.h =
                        Some(attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?)
                }
                expanded_name!("", "viewBox") => self.vbox = attr.parse(value).map(Some)?,
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
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        let params = draw_ctx.get_view_params();

        let has_parent = node.parent().is_some();

        let clip_mode = if !values.is_overflow() && has_parent {
            Some(ClipMode::ClipToViewport)
        } else {
            None
        };

        let svg_viewport = self.get_viewport(values, &params, !has_parent);

        let is_measuring_toplevel_svg = !has_parent && draw_ctx.is_measuring();

        let (viewport, vbox) = if is_measuring_toplevel_svg || has_parent {
            // We are obtaining the toplevel SVG's geometry.  This means, don't care about the
            // DrawingCtx's viewport, just use the SVG's intrinsic dimensions and see how far
            // it wants to extend.
            (svg_viewport, self.vbox)
        } else {
            (
                // The client's viewport overrides the toplevel's x/y/w/h viewport
                draw_ctx.toplevel_viewport(),
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

        draw_ctx.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
            let _params =
                dc.push_new_viewport(vbox, viewport, self.preserve_aspect_ratio, clip_mode);

            node.draw_children(an, cascaded, dc, clipping)
        })
    }
}

#[derive(Default)]
pub struct Use {
    link: Option<Fragment>,
    x: Length<Horizontal>,
    y: Length<Vertical>,
    w: Option<Length<Horizontal>>,
    h: Option<Length<Vertical>>,
}

impl Use {
    pub fn get_rect(&self, values: &ComputedValues, params: &ViewParams) -> Rect {
        let x = self.x.normalize(values, &params);
        let y = self.y.normalize(values, &params);

        // If attributes ‘width’ and/or ‘height’ are not specified,
        // [...] use values of '100%' for these attributes.
        // From https://www.w3.org/TR/SVG/struct.html#UseElement in
        // "If the ‘use’ element references a ‘symbol’ element"

        let w = self
            .w
            .unwrap_or_else(|| Length::<Horizontal>::parse_str("100%").unwrap())
            .normalize(values, &params);
        let h = self
            .h
            .unwrap_or_else(|| Length::<Vertical>::parse_str("100%").unwrap())
            .normalize(values, &params);

        Rect::new(x, y, x + w, y + h)
    }
}

impl SetAttributes for Use {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                ref a if is_href(a) => set_href(
                    a,
                    &mut self.link,
                    Fragment::parse(value).attribute(attr.clone())?,
                ),

                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => {
                    self.w = attr
                        .parse_and_validate(value, Length::<Horizontal>::check_nonnegative)
                        .map(Some)?
                }
                expanded_name!("", "height") => {
                    self.h = attr
                        .parse_and_validate(value, Length::<Vertical>::check_nonnegative)
                        .map(Some)?
                }
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
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        draw_ctx.draw_from_use_node(node, acquired_nodes, cascaded, self.link.as_ref(), clipping)
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
                expanded_name!("", "viewBox") => self.vbox = attr.parse(value).map(Some)?,
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
    width: Length<Horizontal>,
    height: Length<Vertical>,

    units: MaskUnits,
    content_units: MaskContentUnits,
}

impl Default for Mask {
    fn default() -> Mask {
        Mask {
            // these values are per the spec
            x: Length::<Horizontal>::parse_str("-10%").unwrap(),
            y: Length::<Vertical>::parse_str("-10%").unwrap(),
            width: Length::<Horizontal>::parse_str("120%").unwrap(),
            height: Length::<Vertical>::parse_str("120%").unwrap(),

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

    pub fn get_rect(&self, values: &ComputedValues, params: &ViewParams) -> Rect {
        let x = self.x.normalize(&values, &params);
        let y = self.y.normalize(&values, &params);
        let w = self.width.normalize(&values, &params);
        let h = self.height.normalize(&values, &params);

        Rect::new(x, y, x + w, y + h)
    }
}

impl SetAttributes for Mask {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => {
                    self.width =
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?
                }
                expanded_name!("", "height") => {
                    self.height =
                        attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?
                }
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
    link: Option<String>,
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
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let cascaded = CascadedValues::new(cascaded, node);
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, acquired_nodes, values, clipping, &mut |an, dc| {
            match self.link.as_ref() {
                Some(l) if !l.is_empty() => {
                    dc.with_link_tag(l, &mut |dc| node.draw_children(an, &cascaded, dc, clipping))
                }
                _ => node.draw_children(an, &cascaded, dc, clipping),
            }
        })
    }
}
