use cairo::Rectangle;
use markup5ever::local_name;

use crate::allowed_url::Fragment;
use crate::aspect_ratio::*;
use crate::bbox::BoundingBox;
use crate::dpi::Dpi;
use crate::drawing_ctx::{ClipMode, DrawingCtx, ViewParams};
use crate::error::{AcquireError, AttributeResultExt, RenderingError};
use crate::float_eq_cairo::ApproxEqCairo;
use crate::length::*;
use crate::node::*;
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::property_bag::PropertyBag;
use crate::property_defs::Overflow;
use crate::rect::RectangleExt;
use crate::viewbox::*;

#[derive(Default)]
pub struct NodeGroup();

impl NodeTrait for NodeGroup {
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
pub struct NodeNonRendering;

impl NodeTrait for NodeNonRendering {
    fn set_atts(&mut self, _: Option<&RsvgNode>, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }
}

#[derive(Default)]
pub struct NodeSwitch();

impl NodeTrait for NodeSwitch {
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
    pub width: Option<LengthHorizontal>,
    pub height: Option<LengthVertical>,
    pub vbox: Option<ViewBox>,
}

#[derive(Default)]
pub struct NodeSvg {
    preserve_aspect_ratio: AspectRatio,
    x: Option<LengthHorizontal>,
    y: Option<LengthVertical>,
    w: Option<LengthHorizontal>,
    h: Option<LengthVertical>,
    vbox: Option<ViewBox>,
}

impl NodeSvg {
    pub fn get_size(&self, values: &ComputedValues, dpi: Dpi) -> Option<(i32, i32)> {
        let (_, _, w, h) = self.get_unnormalized_viewport();

        match (w, h, self.vbox) {
            (w, h, Some(vbox)) => {
                let params = ViewParams::new(dpi.x(), dpi.y(), vbox.width, vbox.height);

                Some((
                    w.normalize(values, &params).round() as i32,
                    h.normalize(values, &params).round() as i32,
                ))
            }

            (w, h, None) if w.unit() != LengthUnit::Percent && h.unit() != LengthUnit::Percent => {
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
            width: self.w,
            height: self.h,
            vbox: self.vbox,
        }
    }

    // returns (x, y, w, h)
    fn get_unnormalized_viewport(
        &self,
    ) -> (
        LengthHorizontal,
        LengthVertical,
        LengthHorizontal,
        LengthVertical,
    ) {
        // these defaults are per the spec
        let x = self
            .x
            .unwrap_or_else(|| LengthHorizontal::parse_str("0").unwrap());
        let y = self
            .y
            .unwrap_or_else(|| LengthVertical::parse_str("0").unwrap());
        let w = self
            .w
            .unwrap_or_else(|| LengthHorizontal::parse_str("100%").unwrap());
        let h = self
            .h
            .unwrap_or_else(|| LengthVertical::parse_str("100%").unwrap());

        (x, y, w, h)
    }

    fn get_viewport(&self, values: &ComputedValues, params: &ViewParams) -> Rectangle {
        let (x, y, w, h) = self.get_unnormalized_viewport();

        Rectangle::new(
            x.normalize(values, &params),
            y.normalize(values, &params),
            w.normalize(values, &params),
            h.normalize(values, &params),
        )
    }
}

impl NodeTrait for NodeSvg {
    fn set_atts(&mut self, parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        let is_inner_svg = parent.is_some();

        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("preserveAspectRatio") => {
                    self.preserve_aspect_ratio = attr.parse(value)?
                }
                local_name!("x") if is_inner_svg => self.x = Some(attr.parse(value)?),
                local_name!("y") if is_inner_svg => self.y = Some(attr.parse(value)?),
                local_name!("width") => {
                    self.w =
                        Some(attr.parse_and_validate(value, LengthHorizontal::check_nonnegative)?)
                }
                local_name!("height") => {
                    self.h =
                        Some(attr.parse_and_validate(value, LengthVertical::check_nonnegative)?)
                }
                local_name!("viewBox") => self.vbox = attr.parse(value).map(Some)?,
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
                    Some(ViewBox {
                        x: 0.0,
                        y: 0.0,
                        width: svg_viewport.width,
                        height: svg_viewport.height,
                    })
                }),
            )
        };

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            let _params =
                dc.push_new_viewport(vbox, &viewport, self.preserve_aspect_ratio, clip_mode);

            node.draw_children(cascaded, dc, clipping)
        })
    }
}

#[derive(Default)]
pub struct NodeUse {
    link: Option<Fragment>,
    x: LengthHorizontal,
    y: LengthVertical,
    w: Option<LengthHorizontal>,
    h: Option<LengthVertical>,
}

impl NodeTrait for NodeUse {
    fn set_atts(&mut self, _: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("xlink:href") => {
                    self.link = Some(Fragment::parse(value).attribute(attr)?)
                }
                local_name!("x") => self.x = attr.parse(value)?,
                local_name!("y") => self.y = attr.parse(value)?,
                local_name!("width") => {
                    self.w = attr
                        .parse_and_validate(value, LengthHorizontal::check_nonnegative)
                        .map(Some)?
                }
                local_name!("height") => {
                    self.h = attr
                        .parse_and_validate(value, LengthVertical::check_nonnegative)
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

        let acquired = match draw_ctx.acquire_node(link, &[]) {
            Ok(acquired) => acquired,

            Err(AcquireError::CircularReference(_)) => {
                // FIXME: add a fragment or node id to this:
                rsvg_log!("circular reference in <use> element {}", node);
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

        if node.ancestors().any(|ancestor| ancestor == *child) {
            // or, if we're <use>'ing ourselves
            return Err(RenderingError::CircularReference);
        }

        let params = draw_ctx.get_view_params();

        let nx = self.x.normalize(values, &params);
        let ny = self.y.normalize(values, &params);

        // If attributes ‘width’ and/or ‘height’ are not specified,
        // [...] use values of '100%' for these attributes.
        // From https://www.w3.org/TR/SVG/struct.html#UseElement in
        // "If the ‘use’ element references a ‘symbol’ element"

        let nw = self
            .w
            .unwrap_or_else(|| LengthHorizontal::parse_str("100%").unwrap())
            .normalize(values, &params);
        let nh = self
            .h
            .unwrap_or_else(|| LengthVertical::parse_str("100%").unwrap())
            .normalize(values, &params);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if nw.approx_eq_cairo(0.0) || nh.approx_eq_cairo(0.0) {
            return Ok(draw_ctx.empty_bbox());
        }

        let viewport = Rectangle::new(nx, ny, nw, nh);

        if child.borrow().get_type() != NodeType::Symbol {
            let cr = draw_ctx.get_cairo_context();
            cr.translate(viewport.x, viewport.y);

            draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
                dc.draw_node_from_stack(
                    &CascadedValues::new_from_values(&child, values),
                    &child,
                    clipping,
                )
            })
        } else {
            let node_data = child.borrow();
            let symbol = node_data.get_impl::<NodeSymbol>();

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
                    &viewport,
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
pub struct NodeSymbol {
    preserve_aspect_ratio: AspectRatio,
    vbox: Option<ViewBox>,
}

impl NodeTrait for NodeSymbol {
    fn set_atts(&mut self, _parent: Option<&RsvgNode>, pbag: &PropertyBag<'_>) -> NodeResult {
        for (attr, value) in pbag.iter() {
            match attr {
                local_name!("preserveAspectRatio") => {
                    self.preserve_aspect_ratio = attr.parse(value)?
                }
                local_name!("viewBox") => self.vbox = attr.parse(value).map(Some)?,
                _ => (),
            }
        }

        Ok(())
    }

    fn overflow_hidden(&self) -> bool {
        true
    }
}
