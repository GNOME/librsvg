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

/// The `<image>` element.
///
/// Note that its x/y/width/height are properties in SVG2, so they are
/// defined as part of [the properties machinery](properties.rs).
#[derive(Default)]
pub struct Image {
    aspect: AspectRatio,
    href: Option<String>,
}

impl SetAttributes for Image {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
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

        let x = values.x().0.to_user(&params);
        let y = values.y().0.to_user(&params);

        let w = match values.width().0 {
            LengthOrAuto::Length(l) => l.to_user(&params),
            LengthOrAuto::Auto => surface.width() as f64,
        };
        let h = match values.height().0 {
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
