//! The `image` element.

use markup5ever::{expanded_name, local_name, namespace_url, ns};

use crate::allowed_url::Href;
use crate::aspect_ratio::AspectRatio;
use crate::attributes::Attributes;
use crate::bbox::BoundingBox;
use crate::document::AcquiredNodes;
use crate::drawing_ctx::DrawingCtx;
use crate::element::{Draw, ElementResult, SetAttributes};
use crate::error::*;
use crate::href::{is_href, set_href};
use crate::length::*;
use crate::node::{CascadedValues, Node};
use crate::parsers::ParseValue;
use crate::rect::Rect;

#[derive(Default)]
pub struct Image {
    x: Length<Horizontal>,
    y: Length<Vertical>,
    width: Length<Horizontal>,
    height: Length<Vertical>,
    aspect: AspectRatio,
    href: Option<Href>,
}

impl SetAttributes for Image {
    fn set_attributes(&mut self, attrs: &Attributes) -> ElementResult {
        for (attr, value) in attrs.iter() {
            match attr.expanded() {
                expanded_name!("", "x") => self.x = attr.parse(value)?,
                expanded_name!("", "y") => self.y = attr.parse(value)?,
                expanded_name!("", "width") => {
                    self.width = attr.parse_and_validate(value, Length::check_nonnegative)?
                }
                expanded_name!("", "height") => {
                    self.height = attr.parse_and_validate(value, Length::check_nonnegative)?
                }
                expanded_name!("", "preserveAspectRatio") => self.aspect = attr.parse(value)?,

                // "path" is used by some older Adobe Illustrator versions
                ref a if is_href(a) || *a == expanded_name!("", "path") => {
                    let href = Href::parse(value)
                        .map_err(|_| ValueErrorKind::parse_error("could not parse href"))
                        .attribute(attr.clone())?;

                    set_href(a, &mut self.href, href);
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
        acquired_nodes: &mut AcquiredNodes,
        cascaded: &CascadedValues<'_>,
        draw_ctx: &mut DrawingCtx,
        clipping: bool,
    ) -> Result<BoundingBox, RenderingError> {
        let surface = match self.href {
            Some(Href::PlainUrl(ref url)) => match acquired_nodes.lookup_image(url) {
                Ok(surf) => surf,
                Err(e) => {
                    rsvg_log!("could not load image \"{}\": {}", url, e);
                    return Ok(draw_ctx.empty_bbox());
                }
            },
            Some(_) => {
                rsvg_log!(
                    "not rendering {} because its href cannot contain a fragment identifier",
                    node
                );

                return Ok(draw_ctx.empty_bbox());
            }
            None => return Ok(draw_ctx.empty_bbox()),
        };

        let values = cascaded.get();

        let params = draw_ctx.get_view_params();
        let x = self.x.normalize(&values, &params);
        let y = self.y.normalize(&values, &params);
        let w = self.width.normalize(&values, &params);
        let h = self.height.normalize(&values, &params);

        draw_ctx.draw_image(
            &surface,
            Rect::new(x, y, x + w, y + h),
            self.aspect,
            node,
            acquired_nodes,
            values,
            clipping,
        )
    }
}
