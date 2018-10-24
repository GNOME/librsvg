use std::cell::Cell;
use std::cell::RefCell;

use glib::translate::*;
use glib_sys;

use aspect_ratio::*;
use attributes::Attribute;
use drawing_ctx::DrawingCtx;
use error::RenderingError;
use float_eq_cairo::ApproxEqCairo;
use handle::RsvgHandle;
use length::*;
use libc;
use node::*;
use parsers::{parse, parse_and_validate, Parse};
use property_bag::{OwnedPropertyBag, PropertyBag};
use state::Overflow;
use viewbox::*;
use viewport::{draw_in_viewport, ClipMode};

pub struct NodeGroup();

impl NodeGroup {
    pub fn new() -> NodeGroup {
        NodeGroup()
    }
}

impl NodeTrait for NodeGroup {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            node.draw_children(cascaded, dc, clipping)
        })
    }
}

pub struct NodeDefs();

impl NodeDefs {
    pub fn new() -> NodeDefs {
        NodeDefs()
    }
}

impl NodeTrait for NodeDefs {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }
}

pub struct NodeSwitch();

impl NodeSwitch {
    pub fn new() -> NodeSwitch {
        NodeSwitch()
    }
}

impl NodeTrait for NodeSwitch {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, _: &PropertyBag<'_>) -> NodeResult {
        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let values = cascaded.get();

        draw_ctx.with_discrete_layer(node, values, clipping, &mut |dc| {
            if let Some(child) = node.children().find(|c| c.get_cond()) {
                dc.draw_node_from_stack(&CascadedValues::new(cascaded, &child), &child, clipping)
            } else {
                Ok(())
            }
        })
    }
}

pub struct NodeSvg {
    preserve_aspect_ratio: Cell<AspectRatio>,
    x: Cell<Length>,
    y: Cell<Length>,
    w: Cell<Length>,
    h: Cell<Length>,
    vbox: Cell<Option<ViewBox>>,
    pbag: RefCell<Option<OwnedPropertyBag>>,
}

impl NodeSvg {
    pub fn new() -> NodeSvg {
        NodeSvg {
            preserve_aspect_ratio: Cell::new(AspectRatio::default()),
            x: Cell::new(Length::parse_str("0", LengthDir::Horizontal).unwrap()),
            y: Cell::new(Length::parse_str("0", LengthDir::Vertical).unwrap()),
            w: Cell::new(Length::parse_str("100%", LengthDir::Horizontal).unwrap()),
            h: Cell::new(Length::parse_str("100%", LengthDir::Vertical).unwrap()),
            vbox: Cell::new(None),
            pbag: RefCell::new(None),
        }
    }

    pub fn set_delayed_style(&self, node: &RsvgNode, handle: *const RsvgHandle) {
        if let Some(owned_pbag) = self.pbag.borrow().as_ref() {
            let pbag = PropertyBag::from_owned(owned_pbag);
            node.set_style(handle, &pbag);
        }
    }
}

impl NodeTrait for NodeSvg {
    fn set_atts(
        &self,
        node: &RsvgNode,
        _: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        // SVG element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        // x & y attributes have no effect on outermost svg
        // http://www.w3.org/TR/SVG/struct.html#SVGElement
        let is_inner_svg = node.get_parent().is_some();

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::PreserveAspectRatio => {
                    self.preserve_aspect_ratio
                        .set(parse("preserveAspectRatio", value, ())?)
                }

                Attribute::X => {
                    if is_inner_svg {
                        self.x.set(parse("x", value, LengthDir::Horizontal)?);
                    }
                }

                Attribute::Y => {
                    if is_inner_svg {
                        self.y.set(parse("y", value, LengthDir::Vertical)?);
                    }
                }

                Attribute::Width => self.w.set(parse_and_validate(
                    "width",
                    value,
                    LengthDir::Horizontal,
                    Length::check_nonnegative,
                )?),

                Attribute::Height => self.h.set(parse_and_validate(
                    "height",
                    value,
                    LengthDir::Vertical,
                    Length::check_nonnegative,
                )?),

                Attribute::ViewBox => self.vbox.set(parse("viewBox", value, ()).map(Some)?),

                _ => (),
            }
        }

        // The "style" sub-element is not loaded yet here, so we need
        // to store other attributes to be applied later.
        *self.pbag.borrow_mut() = Some(pbag.to_owned());

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let values = cascaded.get();

        let params = draw_ctx.get_view_params();

        let nx = self.x.get().normalize(values, &params);
        let ny = self.y.get().normalize(values, &params);
        let nw = self.w.get().normalize(values, &params);
        let nh = self.h.get().normalize(values, &params);

        let do_clip = !values.is_overflow() && node.get_parent().is_some();

        draw_in_viewport(
            nx,
            ny,
            nw,
            nh,
            ClipMode::ClipToViewport,
            do_clip,
            self.vbox.get(),
            self.preserve_aspect_ratio.get(),
            node,
            values,
            draw_ctx.get_cairo_context().get_matrix(),
            draw_ctx,
            clipping,
            &mut |dc| {
                // we don't push a layer because draw_in_viewport() already does it
                node.draw_children(cascaded, dc, clipping)
            },
        )
    }
}

pub struct NodeUse {
    link: RefCell<Option<String>>,
    x: Cell<Length>,
    y: Cell<Length>,
    w: Cell<Option<Length>>,
    h: Cell<Option<Length>>,
}

impl NodeUse {
    pub fn new() -> NodeUse {
        NodeUse {
            link: RefCell::new(None),
            x: Cell::new(Length::default()),
            y: Cell::new(Length::default()),
            w: Cell::new(None),
            h: Cell::new(None),
        }
    }
}

impl NodeTrait for NodeUse {
    fn set_atts(&self, _: &RsvgNode, _: *const RsvgHandle, pbag: &PropertyBag<'_>) -> NodeResult {
        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::XlinkHref => *self.link.borrow_mut() = Some(value.to_owned()),

                Attribute::X => self.x.set(parse("x", value, LengthDir::Horizontal)?),
                Attribute::Y => self.y.set(parse("y", value, LengthDir::Vertical)?),

                Attribute::Width => self.w.set(
                    parse_and_validate(
                        "width",
                        value,
                        LengthDir::Horizontal,
                        Length::check_nonnegative,
                    )
                    .map(Some)?,
                ),
                Attribute::Height => self.h.set(
                    parse_and_validate(
                        "height",
                        value,
                        LengthDir::Vertical,
                        Length::check_nonnegative,
                    )
                    .map(Some)?,
                ),

                _ => (),
            }
        }

        Ok(())
    }

    fn draw(
        &self,
        node: &RsvgNode,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx<'_>,
        clipping: bool,
    ) -> Result<(), RenderingError> {
        let values = cascaded.get();

        let link = self.link.borrow();

        if link.is_none() {
            return Ok(());
        }

        let uri = link.as_ref().unwrap();

        let child = if let Some(acquired) = draw_ctx.get_acquired_node(uri) {
            // Here we clone the acquired child, so that we can drop the AcquiredNode as
            // early as possible.  This is so that the child's drawing method will be able
            // to re-acquire the child for other purposes.
            acquired.get().clone()
        } else {
            rsvg_log!(
                "element {} references nonexistent \"{}\"",
                node.get_human_readable_name(),
                uri,
            );
            return Ok(());
        };

        if Node::is_ancestor(child.clone(), node.clone()) {
            // or, if we're <use>'ing ourselves
            return Err(RenderingError::CircularReference);
        }

        draw_ctx.increase_num_elements_rendered_through_use(1);

        let params = draw_ctx.get_view_params();

        let nx = self.x.get().normalize(values, &params);
        let ny = self.y.get().normalize(values, &params);

        // If attributes ‘width’ and/or ‘height’ are not specified,
        // [...] use values of '100%' for these attributes.
        // From https://www.w3.org/TR/SVG/struct.html#UseElement in
        // "If the ‘use’ element references a ‘symbol’ element"

        let nw = self
            .w
            .get()
            .unwrap_or_else(|| Length::parse_str("100%", LengthDir::Horizontal).unwrap())
            .normalize(values, &params);
        let nh = self
            .h
            .get()
            .unwrap_or_else(|| Length::parse_str("100%", LengthDir::Vertical).unwrap())
            .normalize(values, &params);

        // width or height set to 0 disables rendering of the element
        // https://www.w3.org/TR/SVG/struct.html#UseElementWidthAttribute
        if nw.approx_eq_cairo(&0.0) || nh.approx_eq_cairo(&0.0) {
            return Ok(());
        }

        if child.get_type() != NodeType::Symbol {
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
            child.with_impl(|symbol: &NodeSymbol| {
                let do_clip = !values.is_overflow()
                    || (values.overflow == Overflow::Visible && child.is_overflow());

                draw_in_viewport(
                    nx,
                    ny,
                    nw,
                    nh,
                    ClipMode::ClipToVbox,
                    do_clip,
                    symbol.vbox.get(),
                    symbol.preserve_aspect_ratio.get(),
                    node,
                    values,
                    draw_ctx.get_cairo_context().get_matrix(),
                    draw_ctx,
                    clipping,
                    &mut |dc| {
                        // We don't push a layer because draw_in_viewport() already does it
                        child.draw_children(
                            &CascadedValues::new_from_values(&child, values),
                            dc,
                            clipping,
                        )
                    },
                )
            })
        }
    }
}

pub struct NodeSymbol {
    preserve_aspect_ratio: Cell<AspectRatio>,
    vbox: Cell<Option<ViewBox>>,
}

impl NodeSymbol {
    pub fn new() -> NodeSymbol {
        NodeSymbol {
            preserve_aspect_ratio: Cell::new(AspectRatio::default()),
            vbox: Cell::new(None),
        }
    }
}

impl NodeTrait for NodeSymbol {
    fn set_atts(
        &self,
        node: &RsvgNode,
        _: *const RsvgHandle,
        pbag: &PropertyBag<'_>,
    ) -> NodeResult {
        // symbol element has overflow:hidden
        // https://www.w3.org/TR/SVG/styling.html#UAStyleSheet
        node.set_overflow_hidden();

        for (_key, attr, value) in pbag.iter() {
            match attr {
                Attribute::PreserveAspectRatio => {
                    self.preserve_aspect_ratio
                        .set(parse("preserveAspectRatio", value, ())?)
                }

                Attribute::ViewBox => self.vbox.set(parse("viewBox", value, ()).map(Some)?),

                _ => (),
            }
        }

        Ok(())
    }
}

#[no_mangle]
pub extern "C" fn rsvg_node_svg_get_size(
    raw_node: *const RsvgNode,
    dpi_x: libc::c_double,
    dpi_y: libc::c_double,
    out_width: *mut i32,
    out_height: *mut i32,
) -> glib_sys::gboolean {
    assert!(!raw_node.is_null());
    let node: &RsvgNode = unsafe { &*raw_node };

    assert!(!out_width.is_null());
    assert!(!out_height.is_null());

    node.with_impl(
        |svg: &NodeSvg| match (svg.w.get(), svg.h.get(), svg.vbox.get()) {
            (w, h, Some(vb)) => {
                unsafe {
                    *out_width = w.hand_normalize(dpi_x, vb.0.width, 12.0).round() as i32;
                    *out_height = h.hand_normalize(dpi_y, vb.0.height, 12.0).round() as i32;
                }
                true.to_glib()
            }
            (w, h, None) if w.unit != LengthUnit::Percent && h.unit != LengthUnit::Percent => {
                unsafe {
                    *out_width = w.hand_normalize(dpi_x, 0.0, 12.0).round() as i32;
                    *out_height = h.hand_normalize(dpi_y, 0.0, 12.0).round() as i32;
                }
                true.to_glib()
            }
            (_, _, _) => false.to_glib(),
        },
    )
}
