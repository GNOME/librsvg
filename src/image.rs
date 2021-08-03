//! The `image` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::aspect_ratio::AspectRatio;
use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::*;
use crate::href::{is_href, set_href};
use crate::layout::{self, StackingContext};
use crate::length::*;
use crate::node::{CascadedValues, Node, NodeBorrow};
use crate::parsers::ParseValue;
use crate::rect::Rect;
use crate::xml::Attributes;

#[derive(Default)]
pub struct Image {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: LengthOrAuto<Horizontal>,
    height: LengthOrAuto<Vertical>,
    aspect: AspectRatio,
    href: Option<String>,
}

impl SetAttributes for Image {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => self.width = attr.parse(value)?,
                expanded_name!("", "height") => self.height = attr.parse(value)?,
                expanded_name!("", "preserveAspectRatio") => self.aspect = attr.parse(value)?,

                // "path" is used by some older Adobe Illustrator versions
                ref a if is_href(a) || *a == expanded_name!("", "path") => {
                    set_href(a, &mut self.href, value.to_string())
                }

                _ => (),
            }
        }

        Ok(())
    }
}

impl Draw for Image {
    fn draw(
        &self,
        node: &Node,
        acquired_nodes: &mut AcquiredNodes<'_>,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let surface = match self.href {
            Some(ref url) => match acquired_nodes.lookup_image(url) {
                Ok(surf) => surf,
                Err(e) => {
                    rsvg_log!("could not load image \"{}\": {}", url, e);
                    return Ok(draw_ctx.empty_bbox());
                }
            },
            None => return Ok(draw_ctx.empty_bbox()),
        };

        let values = cascaded.get();

        let view_params = draw_ctx.get_view_params();
        let params = NormalizeParams::new(values, &view_params);

        let x = self.x.to_user(&params);
        let y = self.y.to_user(&params);

        let w = match self.width {
            LengthOrAuto::Length(l) => l.to_user(&params),
            LengthOrAuto::Auto => surface.width() as f64,
        };
        let h = match self.height {
            LengthOrAuto::Length(l) => l.to_user(&params),
            LengthOrAuto::Auto => surface.height() as f64,
        };

        let is_visible = values.is_visible();

        let rect = Rect::new(x, y, x + w, y + h);

        let overflow = values.overflow();

        let image = layout::Image {
            surface,
            is_visible,
            rect,
            aspect: self.aspect,
            overflow,
        };

        let elt = node.borrow_element();
        let stacking_ctx = StackingContext::new(acquired_nodes, &elt, values.transform(), values);

        draw_ctx.draw_image(&image, &stacking_ctx, acquired_nodes, values, clipping)
    }
}
