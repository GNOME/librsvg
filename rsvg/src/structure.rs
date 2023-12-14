//! Structural elements in SVG: the `g`, `switch`, `svg`, `use`, `symbol`, `clip_path`, `mask`, `link` elements.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::aspect_ratio::*;
use crate::bbox::BoundingBox;
use crate::coord_units;
use crate::coord_units::CoordUnits;
use crate::document::{AcquiredNodes, NodeId};
use crate::drawing_ctx::{ClipMode, DrawingCtx, SvgNesting, Viewport};
use crate::element::{set_attribute, ElementData, ElementTrait};
use crate::error::*;
use crate::href::{is_href, set_href};
use crate::layout::StackingContext;
use crate::length::*;
use crate::node::{CascadedValues, Node, NodeBorrow, NodeDraw};
use crate::parsers::{Parse, ParseValue};
use crate::properties::ComputedValues;
use crate::rect::Rect;
use crate::session::Session;
use crate::viewbox::*;
use crate::xml::Attributes;

#[derive(Default)]
pub struct Group();

impl ElementTrait for Group {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        viewport: &Viewport,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        let values = cascaded.get();

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(
            draw_ctx.session(),
            acquired_nodes,
            &elt,
            values.transform(),
            None,
            values,
        );

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            viewport,
            clipping,
            &mut |an, dc| node.draw_children(an, cascaded, viewport, dc, clipping),
        )
    }
}

/// A no-op node that does not render anything
///
/// Sometimes we just need a node that can contain children, but doesn't
/// render itself or its children.  This is just that kind of node.
#[derive(Default)]
pub struct NonRendering;

impl ElementTrait for NonRendering {}

/// The `<switch>` element.
#[derive(Default)]
pub struct Switch();

impl ElementTrait for Switch {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        viewport: &Viewport,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        let values = cascaded.get();

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(
            draw_ctx.session(),
            acquired_nodes,
            &elt,
            values.transform(),
            None,
            values,
        );

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            viewport,
            clipping,
            &mut |an, dc| {
                if let Some(child) = node.children().filter(|c| c.is_element()).find(|c| {
                    let elt = c.borrow_element();
                    elt.get_cond(dc.user_language())
                }) {
                    child.draw(
                        an,
                        &CascadedValues::clone_with_node(cascaded, &child),
                        viewport,
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

    pub fn get_viewbox(&self) -> Option<ViewBox> {
        self.vbox
    }

    pub fn get_preserve_aspect_ratio(&self) -> AspectRatio {
        self.preserve_aspect_ratio
    }

    fn make_svg_viewport(
        &self,
        node: &Node,
        cascaded: &CascadedValues<'_>,
        current_viewport: &Viewport,
        draw_ctx: &mut DrawingCtx,
    ) -> Option<Viewport> {
        let values = cascaded.get();

        let params = NormalizeParams::new(values, current_viewport);

        let has_parent = node.parent().is_some();

        // FIXME: do we need to look at preserveAspectRatio.slice, like in DrawingCtx::draw_image()?
        let clip_mode = if !values.is_overflow() && has_parent {
            ClipMode::ClipToViewport
        } else {
            ClipMode::NoClip
        };

        // From https://www.w3.org/TR/SVG2/embedded.html#ImageElement:
        //
        // For `image` elements embedding an SVG image, the `preserveAspectRatio`
        // attribute on the root element in the referenced SVG image must be ignored,
        // and instead treated as if it had a value of `none`. (see
        // `preserveAspectRatio` for details).  This ensures that the
        // `preserveAspectRatio` attribute on the referencing `image` has its
        // intended effect, even if it is none.
        //
        let preserve_aspect_ratio = match (has_parent, draw_ctx.svg_nesting()) {
            // we are a toplevel, and referenced from <image> => preserveAspectRatio=none
            (false, SvgNesting::ReferencedFromImageElement) => AspectRatio::none(),

            // otherwise just use our specified preserveAspectRatio
            _ => self.preserve_aspect_ratio,
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

        draw_ctx.push_new_viewport(
            current_viewport,
            vbox,
            viewport,
            preserve_aspect_ratio,
            clip_mode,
        )
    }
}

impl ElementTrait for Svg {
    fn set_attributes(&mut self, attrs: &Attributes, session: &Session) {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => {
                    set_attribute(&mut self.preserve_aspect_ratio, attr.parse(value), session)
                }
                expanded_name!("", "viewBox") => {
                    set_attribute(&mut self.vbox, attr.parse(value), session)
                }
                _ => (),
            }
        }
    }

    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        viewport: &Viewport,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        let values = cascaded.get();

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(
            draw_ctx.session(),
            acquired_nodes,
            &elt,
            values.transform(),
            None,
            values,
        );

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            viewport, // FIXME: should this be the svg_viewport from below?
            clipping,
            &mut |an, dc| {
                if let Some(svg_viewport) = self.make_svg_viewport(node, cascaded, viewport, dc) {
                    node.draw_children(an, cascaded, &svg_viewport, dc, clipping)
                } else {
                    Ok(dc.empty_bbox())
                }
            },
        )
    }
}

/// The `<use>` element.
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

impl ElementTrait for Use {
    fn set_attributes(&mut self, attrs: &Attributes, session: &Session) {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                ref a if is_href(a) => {
                    let mut href = None;
                    set_attribute(
                        &mut href,
                        NodeId::parse(value).map(Some).attribute(attr.clone()),
                        session,
                    );
                    set_href(a, &mut self.link, href);
                }
                expanded_name!("", "x") => set_attribute(&mut self.x, attr.parse(value), session),
                expanded_name!("", "y") => set_attribute(&mut self.y, attr.parse(value), session),
                expanded_name!("", "width") => {
                    set_attribute(&mut self.width, attr.parse(value), session)
                }
                expanded_name!("", "height") => {
                    set_attribute(&mut self.height, attr.parse(value), session)
                }
                _ => (),
            }
        }
    }

    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        viewport: &Viewport,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        if let Some(link) = self.link.as_ref() {
            let values = cascaded.get();
            let params = NormalizeParams::new(values, viewport);
            let rect = self.get_rect(&params);

            let stroke_paint = values.stroke().0.resolve(
                acquired_nodes,
                values.stroke_opacity().0,
                values.color().0,
                cascaded.context_fill.clone(),
                cascaded.context_stroke.clone(),
                draw_ctx.session(),
            );

            let fill_paint = values.fill().0.resolve(
                acquired_nodes,
                values.fill_opacity().0,
                values.color().0,
                cascaded.context_fill.clone(),
                cascaded.context_stroke.clone(),
                draw_ctx.session(),
            );

            draw_ctx.draw_from_use_node(
                node,
                acquired_nodes,
                values,
                rect,
                link,
                clipping,
                viewport,
                fill_paint,
                stroke_paint,
            )
        } else {
            Ok(draw_ctx.empty_bbox())
        }
    }
}

/// The `<symbol>` element.
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

impl ElementTrait for Symbol {
    fn set_attributes(&mut self, attrs: &Attributes, session: &Session) {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "preserveAspectRatio") => {
                    set_attribute(&mut self.preserve_aspect_ratio, attr.parse(value), session)
                }
                expanded_name!("", "viewBox") => {
                    set_attribute(&mut self.vbox, attr.parse(value), session)
                }
                _ => (),
            }
        }
    }
}

coord_units!(ClipPathUnits, CoordUnits::UserSpaceOnUse);

/// The `<clipPath>` element.
#[derive(Default)]
pub struct ClipPath {
    units: ClipPathUnits,
}

impl ClipPath {
    pub fn get_units(&self) -> CoordUnits {
        CoordUnits::from(self.units)
    }
}

impl ElementTrait for ClipPath {
    fn set_attributes(&mut self, attrs: &Attributes, session: &Session) {
        for (attr, value) in attrs.iter() {
            if attr.expanded() == expanded_name!("", "clipPathUnits") {
                set_attribute(&mut self.units, attr.parse(value), session);
            }
        }
    }
}

coord_units!(MaskUnits, CoordUnits::ObjectBoundingBox);
coord_units!(MaskContentUnits, CoordUnits::UserSpaceOnUse);

/// The `<mask>` element.
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

impl ElementTrait for Mask {
    fn set_attributes(&mut self, attrs: &Attributes, session: &Session) {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => set_attribute(&mut self.x, attr.parse(value), session),
                expanded_name!("", "y") => set_attribute(&mut self.y, attr.parse(value), session),
                expanded_name!("", "width") => {
                    set_attribute(&mut self.width, attr.parse(value), session)
                }
                expanded_name!("", "height") => {
                    set_attribute(&mut self.height, attr.parse(value), session)
                }
                expanded_name!("", "maskUnits") => {
                    set_attribute(&mut self.units, attr.parse(value), session)
                }
                expanded_name!("", "maskContentUnits") => {
                    set_attribute(&mut self.content_units, attr.parse(value), session)
                }
                _ => (),
            }
        }
    }
}

/// The `<a>` element.
#[derive(Default)]
pub struct Link {
    pub link: Option<String>,
}

impl ElementTrait for Link {
    fn set_attributes(&mut self, attrs: &Attributes, _session: &Session) {
        for (attr, value) in attrs.iter() {
            let expanded = attr.expanded();
            if is_href(&expanded) {
                set_href(&expanded, &mut self.link, Some(value.to_owned()));
            }
        }
    }

    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        viewport: &Viewport,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, InternalRenderingError> {
        // If this element is inside of <text>, do not draw it.
        // The <text> takes care of it.
        for an in node.ancestors() {
            if matches!(&*an.borrow_element_data(), ElementData::Text(_)) {
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
            draw_ctx.session(),
            acquired_nodes,
            &elt,
            values.transform(),
            values,
            link_target,
        );

        draw_ctx.with_discrete_layer(
            &stacking_ctx,
            acquired_nodes,
            viewport,
            clipping,
            &mut |an, dc| node.draw_children(an, &cascaded, viewport, dc, clipping),
        )
    }
}
