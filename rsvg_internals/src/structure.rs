//! Structural elements in SVG: the `g`, `switch`, `svg`, `use`, `symbol`, `clip_path`, `mask`, `link` elements.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::allowed_url::Fragment;
use crate::aspect_ratio::*;
use crate::bbox::BoundingBox;
use crate::coord_units::CoordUnits;
use crate::dpi::Dpi;
use crate::drawing_ctx::{ClipMode, DrawingCtx, ViewParams};
use crate::error::*;
use crate::float_eq_cairo::ApproxEqCairo;
use crate::length::*;
use crate::node::*;
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::property_defs::Overflow;
use crate::rect::Rect;
use crate::viewbox::*;

#[derive(Default)]
pub struct Group();

impl NodeTrait for Group {
    fn set_atts(&mut self, _: Option<&RsvgNode>, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            node.draw_children(cascaded, dc, clipping)
        })
    }
}

/// A no-op node that does not render anything
///
/// Sometimes we just need a node that can contain children, but doesn't
/// render itself or its children.  This is just that kind of node.
#[derive(Default)]
pub struct NonRendering;

impl NodeTrait for NonRendering {
    fn set_atts(&mut self, _: Option<&RsvgNode>, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }
}

#[derive(Default)]
pub struct Switch();

impl NodeTrait for Switch {
    fn set_atts(&mut self, _: Option<&RsvgNode>, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            if let Some(child) = node
                .children()
                .filter(|c| c.borrow().get_type() != NodeType::Chars)
                .find(|c| c.borrow().get_cond())
            {
                dc.draw_node_from_stack(&CascadedValues::new(cascaded, &child), &child, clipping)
            } else {
                Ok(dc.empty_bbox())
            }
        })
    }
}

/// Intrinsic dimensions of an SVG document fragment
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct IntrinsicDimensions {
    pub width: Option<Length<Horizontal>>,
    pub height: Option<Length<Vertical>>,
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
    pub fn get_size(&self, values: &ComputedValues, dpi: Dpi) -> Option<(i32, i32)> {
        let (_, _, w, h) = self.get_unnormalized_viewport();

        match (w, h, self.vbox) {
            (w, h, Some(vbox)) => {
                let params = ViewParams::new(dpi.x(), dpi.y(), vbox.0.width(), vbox.0.height());

                Some((
                    w.normalize(values, &params).round() as i32,
                    h.normalize(values, &params).round() as i32,
                ))
            }

            (w, h, None) if w.unit != LengthUnit::Percent && h.unit != LengthUnit::Percent => {
                let params = ViewParams::new(dpi.x(), dpi.y(), 0.0, 0.0);

                Some((
                    w.normalize(values, &params).round() as i32,
                    h.normalize(values, &params).round() as i32,
                ))
            }
            (_, _, _) => None,
        }
    }

    pub fn get_intrinsic_dimensions(&self) -> IntrinsicDimensions {
        IntrinsicDimensions {
            width: self.w.map(Into::into),
            height: self.h.map(Into::into),
            vbox: self.vbox,
        }
    }

    // returns (x, y, w, h)
    fn get_unnormalized_viewport(
        &self,
    ) -> (
        Length<Horizontal>,
        Length<Vertical>,
        Length<Horizontal>,
        Length<Vertical>,
    ) {
        // these defaults are per the spec
        let x = self
            .x
            .unwrap_or_else(|| Length::<Horizontal>::parse_str("0").unwrap());
        let y = self
            .y
            .unwrap_or_else(|| Length::<Vertical>::parse_str("0").unwrap());
        let w = self
            .w
            .unwrap_or_else(|| Length::<Horizontal>::parse_str("100%").unwrap());
        let h = self
            .h
            .unwrap_or_else(|| Length::<Vertical>::parse_str("100%").unwrap());

        (x, y, w, h)
    }

    fn get_viewport(&self, values: &ComputedValues, params: &ViewParams) -> Rect {
        let (x, y, w, h) = self.get_unnormalized_viewport();

        let nx = x.normalize(values, &params);
        let ny = y.normalize(values, &params);
        let nw = w.normalize(values, &params);
        let nh = h.normalize(values, &params);

        Rect::new(nx, ny, nx + nw, ny + nh)
    }
}

impl NodeTrait for Svg {
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        let is_inner_svg = parent.is_some();

        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "preserveAspectRatio") => {
                    self.preserve_aspect_ratio = attr.parse(value)?
                }
                expanded_name!(svg "x") if is_inner_svg => self.x = Some(attr.parse(value)?),
                expanded_name!(svg "y") if is_inner_svg => self.y = Some(attr.parse(value)?),
                expanded_name!(svg "width") => {
                    self.w = Some(
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?,
                    )
                }
                expanded_name!(svg "height") => {
                    self.h =
                        Some(attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?)
                }
                expanded_name!(svg "viewBox") => self.vbox = attr.parse(value).map(Some)?,
                _ => (),
            }
        }

        Ok(())
    }

    fn overflow_hidden(&self) -> bool {
        true
    }

    fn draw(
        &self,
        node: &RsvgNode,
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

        let svg_viewport = self.get_viewport(values, &params);

        let is_measuring_toplevel_svg = !has_parent && draw_ctx.is_measuring();

        let (viewport, vbox) = if is_measuring_toplevel_svg {
            // We are obtaining the toplevel SVG's geometry.  This means, don't care about the
            // DrawingCtx's viewport, just use the SVG's intrinsic dimensions and see how far
            // it wants to extend.
            (svg_viewport, self.vbox)
        } else if has_parent {
            (svg_viewport, self.vbox)
        } else {
            (
                // The client's viewport overrides the toplevel's x/y/w/h viewport
                draw_ctx.toplevel_viewport(),
                // Use our viewBox if available, or try to derive one from
                // the intrinsic dimensions.
                self.vbox.or_else(|| {
                    Some(ViewBox(Rect::from_size(
                        svg_viewport.width(),
                        svg_viewport.height(),
                    )))
                }),
            )
        };

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            let _params =
                dc.push_new_viewport(vbox, viewport, self.preserve_aspect_ratio, clip_mode);

            node.draw_children(cascaded, dc, clipping)
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

impl NodeTrait for Use {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(xlink "href") => {
                    self.link = Some(Fragment::parse(value).attribute(attr)?)
                }
                expanded_name!(svg "x") => self.x = attr.parse(value)?,
                expanded_name!(svg "y") => self.y = attr.parse(value)?,
                expanded_name!(svg "width") => {
                    self.w = attr
                        .parse_and_validate(value, Length::<Horizontal>::check_nonnegative)
                        .map(Some)?
                }
                expanded_name!(svg "height") => {
                    self.h = attr
                        .parse_and_validate(value, Length::<Vertical>::check_nonnegative)
                        .map(Some)?
                }
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let values = cascaded.get();

        if self.link.is_none() {
            return Ok(draw_ctx.empty_bbox());
        }

        let link = self.link.as_ref().unwrap();

        // <use> is an element that is used directly, unlike
        // <pattern>, which is used through a fill="url(#...)"
        // reference.  However, <use> will always reference another
        // element, potentially itself or an ancestor of itself (or
        // another <use> which references the first one, etc.).  So,
        // we acquire the <use> element itself so that circular
        // references can be caught.
        let _self_acquired = draw_ctx.acquire_node_ref(node).map_err(|e| {
            if let AcquireError::CircularReference(_) = e {
                rsvg_log!("circular reference in element {}", node);
                RenderingError::CircularReference
            } else {
                unreachable!();
            }
        })?;

        let acquired = match draw_ctx.acquire_node(link, &[]) {
            Ok(acquired) => acquired,

            Err(AcquireError::CircularReference(node)) => {
                rsvg_log!("circular reference in element {}", node);
                return Err(RenderingError::CircularReference);
            }

            Err(AcquireError::MaxReferencesExceeded) => {
                return Err(RenderingError::InstancingLimit);
            }

            Err(AcquireError::InvalidLinkType(_)) => unreachable!(),

            Err(AcquireError::LinkNotFound(fragment)) => {
                rsvg_log!("element {} references nonexistent \"{}\"", node, fragment);
                return Ok(draw_ctx.empty_bbox());
            }
        };

        let child = acquired.get();

        let params = draw_ctx.get_view_params();

        let nx = self.x.normalize(values, &params);
        let ny = self.y.normalize(values, &params);

        // If attributes ‘width’ and/or ‘height’ are not specified,
        // [...] use values of '100%' for these attributes.
        // From https://www.w3.org/TR/SVG/struct.html#UseElement in
        // "If the ‘use’ element references a ‘symbol’ element"

        let nw = self
            .w
            .unwrap_or_else(|| Length::<Horizontal>::parse_str("100%").unwrap())
            .normalize(values, &params);
        let nh = self
            .h
            .unwrap_or_else(|| Length::<Vertical>::parse_str("100%").unwrap())
            .normalize(values, &params);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if nw.approx_eq_cairo(0.0) || nh.approx_eq_cairo(0.0) {
            return Ok(draw_ctx.empty_bbox());
        }

        if child.borrow().get_type() != NodeType::Symbol {
            let cr = draw_ctx.get_cairo_context();
            cr.translate(nx, ny);

            draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
                dc.draw_node_from_stack(
                    &CascadedValues::new_from_values(&child, values),
                    &child,
                    clipping,
                )
            })
        } else {
            let node_data = child.borrow();
            let symbol = node_data.get_impl::<Symbol>();

            let clip_mode = if !values.is_overflow()
                || (values.overflow == Overflow::Visible && child.borrow().is_overflow())
            {
                Some(ClipMode::ClipToVbox)
            } else {
                None
            };

            draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
                let _params = dc.push_new_viewport(
                    symbol.vbox,
                    Rect::new(nx, ny, nx + nw, ny + nh),
                    symbol.preserve_aspect_ratio,
                    clip_mode,
                );

                child.draw_children(
                    &CascadedValues::new_from_values(&child, values),
                    dc,
                    clipping,
                )
            })
        }
    }
}

#[derive(Default)]
pub struct Symbol {
    preserve_aspect_ratio: AspectRatio,
    vbox: Option<ViewBox>,
}

impl NodeTrait for Symbol {
    fn set_atts(&mut self, _parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "preserveAspectRatio") => {
                    self.preserve_aspect_ratio = attr.parse(value)?
                }
                expanded_name!(svg "viewBox") => self.vbox = attr.parse(value).map(Some)?,
                _ => (),
            }
        }

        Ok(())
    }

    fn overflow_hidden(&self) -> bool {
        true
    }
}

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

impl NodeTrait for ClipPath {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "clipPathUnits") => self.units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

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
        CoordUnits::from(self.content_units)
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

impl NodeTrait for Mask {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(svg "x") => self.x = attr.parse(value)?,
                expanded_name!(svg "y") => self.y = attr.parse(value)?,
                expanded_name!(svg "width") => {
                    self.width =
                        attr.parse_and_validate(value, Length::<Horizontal>::check_nonnegative)?
                }
                expanded_name!(svg "height") => {
                    self.height =
                        attr.parse_and_validate(value, Length::<Vertical>::check_nonnegative)?
                }
                expanded_name!(svg "maskUnits") => self.units = attr.parse(value)?,
                expanded_name!(svg "maskContentUnits") => self.content_units = attr.parse(value)?,
                _ => (),
            }
        }

        Ok(())
    }
}

#[derive(Default)]
pub struct Link {
    link: Option<String>,
}

impl NodeTrait for Link {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr.expanded() {
                expanded_name!(xlink "href") => self.link = Some(value.to_owned()),
                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let cascaded = CascadedValues::new(cascaded, node);
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| match self.link.as_ref() {
            Some(l) if !l.is_empty() => {
                dc.with_link_tag(l, &mut |dc| node.draw_children(&cascaded, dc, clipping))
            }
            _ => node.draw_children(&cascaded, dc, clipping),
        })
    }
}
